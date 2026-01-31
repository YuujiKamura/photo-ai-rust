#!/bin/bash
# 過去のコミットから理解度チェック質問を生成
# Usage: ./scripts/review-commit.sh [commit-hash]
#        ./scripts/review-commit.sh          # 最近10件から選択
#        ./scripts/review-commit.sh abc123   # 指定コミット
#        ./scripts/review-commit.sh random   # ランダム選択

# 色定義
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# Claude CLIの確認
if ! command -v claude &> /dev/null; then
    echo -e "${RED}❌ Claude CLI が必要です${NC}"
    echo "   インストール: npm install -g @anthropic-ai/claude-cli"
    exit 1
fi

# 引数処理
TARGET="$1"

if [ -z "$TARGET" ]; then
    # 最近のコミット一覧を表示して選択
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  過去のコミットから理解度チェック${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "${BOLD}最近のコミット:${NC}"
    echo ""

    # 番号付きで表示
    git log --oneline -10 | nl -w2 -s') '

    echo ""
    read -p "番号を選択 (1-10, または 'r' でランダム): " choice

    if [ "$choice" = "r" ] || [ "$choice" = "R" ]; then
        # ランダム選択（最近50件から）
        COMMIT=$(git log --oneline -50 | shuf -n 1 | cut -d' ' -f1)
        echo -e "${YELLOW}ランダム選択: ${COMMIT}${NC}"
    elif [[ "$choice" =~ ^[0-9]+$ ]] && [ "$choice" -ge 1 ] && [ "$choice" -le 10 ]; then
        COMMIT=$(git log --oneline -10 | sed -n "${choice}p" | cut -d' ' -f1)
    else
        echo -e "${RED}無効な選択${NC}"
        exit 1
    fi
elif [ "$TARGET" = "random" ]; then
    # ランダム選択
    COMMIT=$(git log --oneline -50 | shuf -n 1 | cut -d' ' -f1)
    echo -e "${YELLOW}ランダム選択: ${COMMIT}${NC}"
else
    # 指定されたコミット
    COMMIT="$TARGET"
fi

# コミット情報を取得
COMMIT_INFO=$(git log -1 --format="%h %s" "$COMMIT" 2>/dev/null)
if [ -z "$COMMIT_INFO" ]; then
    echo -e "${RED}❌ コミット '$COMMIT' が見つかりません${NC}"
    exit 1
fi

COMMIT_DATE=$(git log -1 --format="%ci" "$COMMIT")
COMMIT_AUTHOR=$(git log -1 --format="%an" "$COMMIT")
COMMIT_MSG=$(git log -1 --format="%s" "$COMMIT")

echo ""
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BOLD}コミット: ${COMMIT}${NC}"
echo -e "メッセージ: ${COMMIT_MSG}"
echo -e "日時: ${COMMIT_DATE}"
echo -e "作者: ${COMMIT_AUTHOR}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# 変更内容を取得
DIFF=$(git show "$COMMIT" --format="" --unified=3)
STATS=$(git show "$COMMIT" --stat --format="")

echo -e "${BOLD}変更ファイル:${NC}"
echo "$STATS"
echo ""

# 変更行数
LINES_CHANGED=$(echo "$DIFF" | grep -c '^[+-]' || echo "0")

if [ "$LINES_CHANGED" -lt 5 ]; then
    echo -e "${YELLOW}⚠ 変更が少なすぎます (${LINES_CHANGED}行)${NC}"
    echo "  別のコミットを選んでください。"
    exit 0
fi

echo -e "Claude CLI で質問を生成中..."
echo ""

# Claude CLIで質問を生成
PROMPT="以下はgitコミットの変更内容です。このコミットを本当に理解しているか確認するための質問を3〜5個作成してください。

コミットメッセージ: $COMMIT_MSG

質問は以下の観点で作成:
1. なぜこの変更が必要だったか
2. この実装方法を選んだ理由
3. 潜在的なバグや副作用はないか
4. 代替案はなかったか
5. この変更が他の部分に与える影響

フォーマット:
- 簡潔な質問文（1行）
- 番号付きリスト

---
$DIFF
---"

# Claude CLI実行
QUESTIONS=$(echo "$PROMPT" | timeout 60 claude -p 2>/dev/null)

if [ -z "$QUESTIONS" ]; then
    echo -e "${RED}❌ 質問生成に失敗しました${NC}"
    exit 1
fi

# 質問を表示
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BOLD}  理解度チェック質問${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "$QUESTIONS"
echo ""
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# オプション
echo ""
echo -e "${BOLD}オプション:${NC}"
echo -e "  ${GREEN}d${NC} - diffを表示"
echo -e "  ${GREEN}a${NC} - 回答例を生成"
echo -e "  ${GREEN}q${NC} - 終了"
echo ""

while true; do
    read -p "選択 [d/a/q]: " -n 1 -r opt
    echo ""

    case $opt in
        [Dd])
            echo ""
            echo -e "${CYAN}━━━ Diff ━━━${NC}"
            echo "$DIFF" | head -100
            if [ $(echo "$DIFF" | wc -l) -gt 100 ]; then
                echo -e "${YELLOW}... (100行以降省略)${NC}"
            fi
            echo ""
            ;;
        [Aa])
            echo ""
            echo -e "回答例を生成中..."

            ANSWER_PROMPT="以下の質問に対する模範回答を簡潔に作成してください。

コミット: $COMMIT_MSG

質問:
$QUESTIONS

変更内容:
$DIFF"

            ANSWERS=$(echo "$ANSWER_PROMPT" | timeout 60 claude -p 2>/dev/null)

            echo ""
            echo -e "${CYAN}━━━ 回答例 ━━━${NC}"
            echo "$ANSWERS"
            echo ""
            ;;
        [Qq])
            echo -e "${GREEN}終了${NC}"
            exit 0
            ;;
        *)
            echo -e "${YELLOW}d/a/q を選択してください${NC}"
            ;;
    esac
done
