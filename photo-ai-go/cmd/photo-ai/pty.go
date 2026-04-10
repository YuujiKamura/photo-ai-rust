package main

import (
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/UserExistsError/conpty"
	"golang.org/x/net/websocket"
)

// ServerPort is set by serve.go before the HTTP server starts.
var ServerPort string

// ---------------------------------------------------------------------------
// Ring Buffer
// ---------------------------------------------------------------------------

// RingBuffer is a circular buffer that stores up to maxLines lines.
type RingBuffer struct {
	mu       sync.Mutex
	lines    []string
	maxLines int
	head     int // next write position
	count    int
}

// NewRingBuffer creates a ring buffer with the given capacity.
func NewRingBuffer(maxLines int) *RingBuffer {
	return &RingBuffer{
		lines:    make([]string, maxLines),
		maxLines: maxLines,
	}
}

// Push adds a line to the ring buffer.
func (rb *RingBuffer) Push(line string) {
	rb.mu.Lock()
	defer rb.mu.Unlock()
	rb.lines[rb.head] = line
	rb.head = (rb.head + 1) % rb.maxLines
	if rb.count < rb.maxLines {
		rb.count++
	}
}

// Last returns the last n lines from the buffer.
func (rb *RingBuffer) Last(n int) []string {
	rb.mu.Lock()
	defer rb.mu.Unlock()
	if n > rb.count {
		n = rb.count
	}
	result := make([]string, n)
	start := (rb.head - n + rb.maxLines) % rb.maxLines
	for i := 0; i < n; i++ {
		result[i] = rb.lines[(start+i)%rb.maxLines]
	}
	return result
}

// All returns all buffered content joined by newlines.
func (rb *RingBuffer) All() string {
	return strings.Join(rb.Last(rb.count), "\n")
}

// Count returns the number of lines currently stored.
func (rb *RingBuffer) Count() int {
	rb.mu.Lock()
	defer rb.mu.Unlock()
	return rb.count
}

// ---------------------------------------------------------------------------
// PTY Session
// ---------------------------------------------------------------------------

type ptySession struct {
	id        string
	cpty      *conpty.ConPty
	buf       *RingBuffer
	shell     string
	createdAt time.Time
}

var (
	ptySessions sync.Map // map[string]*ptySession
	sessionSeq  uint64
	sessionMu   sync.Mutex
)

func nextSessionID() string {
	sessionMu.Lock()
	defer sessionMu.Unlock()
	sessionSeq++
	return fmt.Sprintf("pty-%d", sessionSeq)
}

func getOrCreateSession(cols, rows int) (*ptySession, error) {
	// Return first existing session if any
	var first *ptySession
	ptySessions.Range(func(key, value interface{}) bool {
		first = value.(*ptySession)
		return false
	})
	if first != nil {
		return first, nil
	}
	return createSession(cols, rows)
}

func createSession(cols, rows int) (*ptySession, error) {
	shell := os.Getenv("COMSPEC")
	if shell == "" {
		shell = "cmd.exe"
	}

	homeDir, err := os.UserHomeDir()
	if err != nil {
		homeDir = "."
	}

	cpty, err := conpty.Start(shell,
		conpty.ConPtyDimensions(cols, rows),
		conpty.ConPtyWorkDir(homeDir),
	)
	if err != nil {
		return nil, fmt.Errorf("conpty.Start: %w", err)
	}

	id := nextSessionID()
	sess := &ptySession{
		id:        id,
		cpty:      cpty,
		buf:       NewRingBuffer(1000),
		shell:     shell,
		createdAt: time.Now(),
	}
	ptySessions.Store(id, sess)

	// Background goroutine to populate ring buffer
	go func() {
		buf := make([]byte, 4096)
		var lineBuf strings.Builder
		for {
			n, err := sess.cpty.Read(buf)
			if n > 0 {
				chunk := string(buf[:n])
				for _, ch := range chunk {
					if ch == '\n' {
						sess.buf.Push(lineBuf.String())
						lineBuf.Reset()
					} else {
						lineBuf.WriteRune(ch)
					}
				}
			}
			if err != nil {
				if lineBuf.Len() > 0 {
					sess.buf.Push(lineBuf.String())
				}
				break
			}
		}
	}()

	writeSessionFile(id)
	log.Printf("PTY session created: %s (shell=%s, cols=%d, rows=%d)", id, shell, cols, rows)
	return sess, nil
}

func findSession(sessionID string) *ptySession {
	if sessionID != "" {
		if v, ok := ptySessions.Load(sessionID); ok {
			return v.(*ptySession)
		}
	}
	// Return first session
	var first *ptySession
	ptySessions.Range(func(key, value interface{}) bool {
		first = value.(*ptySession)
		return false
	})
	return first
}

func sessionCount() int {
	count := 0
	ptySessions.Range(func(key, value interface{}) bool {
		count++
		return true
	})
	return count
}

func allSessions() []*ptySession {
	var result []*ptySession
	ptySessions.Range(func(key, value interface{}) bool {
		result = append(result, value.(*ptySession))
		return true
	})
	return result
}

func removeSession(id string) {
	if v, ok := ptySessions.LoadAndDelete(id); ok {
		sess := v.(*ptySession)
		sess.cpty.Close()
		log.Printf("PTY session removed: %s", id)
	}
	deleteSessionFile()
}

// ---------------------------------------------------------------------------
// Session File
// ---------------------------------------------------------------------------

func sessionFilePath() string {
	localAppData := os.Getenv("LOCALAPPDATA")
	if localAppData == "" {
		home, _ := os.UserHomeDir()
		localAppData = filepath.Join(home, "AppData", "Local")
	}
	pid := os.Getpid()
	return filepath.Join(localAppData, "ghostty", "control-plane", "web", "sessions",
		fmt.Sprintf("ghostty-web-%d.session", pid))
}

func writeSessionFile(sessionID string) {
	path := sessionFilePath()
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		log.Printf("Failed to create session dir: %v", err)
		return
	}
	pid := os.Getpid()
	port := ServerPort
	content := fmt.Sprintf("session_name=ghostty-web-%d\nsafe_session_name=ghostty-web\npid=%d\nws_url=ws://localhost:%s/cp\nport=%s\n",
		pid, pid, port, port)
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		log.Printf("Failed to write session file: %v", err)
		return
	}
	log.Printf("Session file written: %s", path)
}

func deleteSessionFile() {
	path := sessionFilePath()
	os.Remove(path)
	log.Printf("Session file deleted: %s", path)
}

// ---------------------------------------------------------------------------
// PTY WebSocket Handler (/ws)
// ---------------------------------------------------------------------------

type resizeMsg struct {
	Type string `json:"type"`
	Cols int    `json:"cols"`
	Rows int    `json:"rows"`
}

func handlePtyWebSocket(ws *websocket.Conn) {
	defer ws.Close()

	origin := ws.Request().Header.Get("Origin")
	if !isAllowedOrigin(origin) {
		log.Printf("PTY WebSocket rejected: invalid origin %q", origin)
		return
	}

	query := ws.Request().URL.Query()
	cols := parseIntDefault(query.Get("cols"), 80)
	rows := parseIntDefault(query.Get("rows"), 24)

	sess, err := getOrCreateSession(cols, rows)
	if err != nil {
		log.Printf("Failed to create PTY session: %v", err)
		return
	}

	log.Printf("PTY WebSocket connected: session=%s remote=%s", sess.id, ws.Request().RemoteAddr)

	// goroutine: PTY output → WebSocket
	done := make(chan struct{})
	go func() {
		defer close(done)
		buf := make([]byte, 4096)
		for {
			n, err := sess.cpty.Read(buf)
			if n > 0 {
				if _, werr := ws.Write(buf[:n]); werr != nil {
					return
				}
			}
			if err != nil {
				return
			}
		}
	}()

	// main loop: WebSocket → PTY
	for {
		var data []byte
		if err := websocket.Message.Receive(ws, &data); err != nil {
			break
		}

		// Check if it's a resize JSON message
		var rm resizeMsg
		if json.Unmarshal(data, &rm) == nil && rm.Type == "resize" && rm.Cols > 0 && rm.Rows > 0 {
			if err := sess.cpty.Resize(rm.Cols, rm.Rows); err != nil {
				log.Printf("PTY resize failed: %v", err)
			}
			continue
		}

		// Otherwise treat as raw input
		if _, err := io.WriteString(sess.cpty, string(data)); err != nil {
			break
		}
	}

	<-done
	log.Printf("PTY WebSocket disconnected: session=%s", sess.id)
}

// ---------------------------------------------------------------------------
// Control Plane WebSocket Handler (/cp)
// ---------------------------------------------------------------------------

var cmdSeq uint64
var cmdSeqMu sync.Mutex

func nextCmdID() string {
	cmdSeqMu.Lock()
	defer cmdSeqMu.Unlock()
	cmdSeq++
	return fmt.Sprintf("cmd_%06d", cmdSeq)
}

func handleCPWebSocket(ws *websocket.Conn) {
	defer ws.Close()

	origin := ws.Request().Header.Get("Origin")
	if !isAllowedOrigin(origin) {
		log.Printf("CP WebSocket rejected: invalid origin %q", origin)
		return
	}

	log.Printf("CP WebSocket connected: %s", ws.Request().RemoteAddr)
	pid := os.Getpid()

	for {
		var msg string
		if err := websocket.Message.Receive(ws, &msg); err != nil {
			break
		}

		msg = strings.TrimSpace(msg)
		if msg == "" {
			continue
		}

		parts := strings.Split(msg, "|")
		action := parts[0]

		var reply string

		switch action {
		case "PING":
			reply = fmt.Sprintf("PONG|ghostty-web|%d", pid)

		case "PERSIST":
			reply = "OK|ghostty-web|PERSIST"

		case "STATE":
			shell := os.Getenv("COMSPEC")
			if shell == "" {
				shell = "cmd.exe"
			}
			count := sessionCount()
			reply = fmt.Sprintf("STATE|ghostty-web|%d|0x0|%s|tab_count=%d|active_tab=0", pid, shell, count)

		case "TAIL":
			n := 20
			var sessionID string
			if len(parts) > 1 {
				if v, err := strconv.Atoi(parts[1]); err == nil {
					n = v
				} else {
					sessionID = parts[1]
				}
			}
			if len(parts) > 2 {
				sessionID = parts[2]
			}
			sess := findSession(sessionID)
			if sess == nil {
				reply = "ERR|ghostty-web|NO_TABS"
			} else {
				lines := sess.buf.Last(n)
				content := strings.Join(lines, "\n")
				reply = fmt.Sprintf("TAIL|ghostty-web|%d\n%s", len(lines), content)
			}

		case "CAPTURE_PANE":
			var sessionID string
			if len(parts) > 1 {
				sessionID = parts[1]
			}
			sess := findSession(sessionID)
			if sess == nil {
				reply = "ERR|ghostty-web|NO_TABS"
			} else {
				content := sess.buf.All()
				lineCount := sess.buf.Count()
				epochMs := time.Now().UnixMilli()
				reply = fmt.Sprintf("OK|ghostty-web|CAPTURE_PANE|epoch_ms=%d|lines=%d\n%s", epochMs, lineCount, content)
			}

		case "LIST_TABS":
			sessions := allSessions()
			count := len(sessions)
			if count == 0 {
				reply = "ERR|ghostty-web|NO_TABS"
			} else {
				var sb strings.Builder
				sb.WriteString(fmt.Sprintf("LIST_TABS|%d|0\n", count))
				for i, s := range sessions {
					sb.WriteString(fmt.Sprintf("TAB|%d|%s|%s|%s\n", i, s.shell, s.id, s.createdAt.Format(time.RFC3339)))
				}
				reply = sb.String()
			}

		case "INPUT":
			if len(parts) < 3 {
				reply = "ERR|ghostty-web|INVALID_ARGS"
				break
			}
			b64text := parts[2]
			var sessionID string
			if len(parts) > 3 {
				sessionID = parts[3]
			}
			decoded, err := base64.StdEncoding.DecodeString(b64text)
			if err != nil {
				reply = "ERR|ghostty-web|DECODE_FAIL"
				break
			}
			sess := findSession(sessionID)
			if sess == nil {
				reply = "ERR|ghostty-web|NO_TABS"
			} else {
				io.WriteString(sess.cpty, string(decoded))
				cmdID := nextCmdID()
				reply = fmt.Sprintf("QUEUED|ghostty-web|INPUT|%s", cmdID)
			}

		case "PASTE":
			if len(parts) < 3 {
				reply = "ERR|ghostty-web|INVALID_ARGS"
				break
			}
			b64text := parts[2]
			var sessionID string
			if len(parts) > 3 {
				sessionID = parts[3]
			}
			decoded, err := base64.StdEncoding.DecodeString(b64text)
			if err != nil {
				reply = "ERR|ghostty-web|DECODE_FAIL"
				break
			}
			sess := findSession(sessionID)
			if sess == nil {
				reply = "ERR|ghostty-web|NO_TABS"
			} else {
				// Bracketed paste
				io.WriteString(sess.cpty, "\x1b[200~"+string(decoded)+"\x1b[201~")
				cmdID := nextCmdID()
				reply = fmt.Sprintf("QUEUED|ghostty-web|PASTE|%s", cmdID)
			}

		case "ACK_POLL":
			if len(parts) < 2 {
				reply = "ERR|ghostty-web|INVALID_ARGS"
			} else {
				reply = fmt.Sprintf("ACK|ghostty-web|%s", parts[1])
			}

		default:
			reply = "ERR|ghostty-web|UNKNOWN_CMD"
		}

		if reply != "" {
			websocket.Message.Send(ws, reply)
		}
	}

	log.Printf("CP WebSocket disconnected: %s", ws.Request().RemoteAddr)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

func parseIntDefault(s string, def int) int {
	if s == "" {
		return def
	}
	v, err := strconv.Atoi(s)
	if err != nil || v <= 0 {
		return def
	}
	return v
}
