//go:build !windows

package main

import (
	"log"

	"golang.org/x/net/websocket"
)

var ServerPort string

func handlePtyWebSocket(ws *websocket.Conn) {
	log.Printf("PTY WebSocket not supported on this platform")
	ws.Close()
}

func handleCPWebSocket(ws *websocket.Conn) {
	log.Printf("CP WebSocket not supported on this platform")
	ws.Close()
}

func writeSessionFile()  {}
func deleteSessionFile() {}
