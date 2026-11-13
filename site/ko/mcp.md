# MCP 연동

`cdg mcp`는 코딩 에이전트가 내가 붙여 넣을 내용을 직접 요청할 수 있게 해 주는 MCP 서버입니다. MCP 레지스트리에는 `io.github.NaNIsaac/codigest`로 올라 있습니다.

## 등록하기

Claude Code:

```bash
claude mcp add --scope user codigest -- cdg mcp
```

다른 클라이언트(Cursor, VS Code, Claude Desktop)는 명령 `cdg`, 인자 `["mcp"]`로 stdio 서버를 추가하면 됩니다. 대부분의 클라이언트는 이런 모양의 설정을 받습니다.

```json
{
  "mcpServers": {
    "codigest": {
      "command": "cdg",
      "args": ["mcp"]
    }
  }
}
```

서버는 실행된 프로젝트를 대상으로 동작합니다. 서버의 작업 폴더가 곧 프로젝트입니다. 서버를 프로젝트 폴더에서 시작하는 클라이언트라면 더 할 일이 없습니다. 다른 곳에서 시작한다면 클라이언트의 서버 설정에서 작업 폴더를 지정하세요.

## 도구

| 도구 | 돌려주는 내용 | 인자 |
| --- | --- | --- |
| `pack` | 마지막 실패와 관련 파일, 변경 사항, import를 예산에 맞춰 | `targets`, `with`, `budget`, `message` |
| `terminal_history` | 최근 명령과 출력, 종료 코드 | `count`, `failed`, `here` |
| `scan` | 프로젝트 전체 묶음 | `path`, `targets`, `resolve`, `message` |
| `diff` | 기준점 이후의 텍스트 변경 | `path`, `message` |
| `semdiff` | 기준점 이후의 함수·클래스 단위 변경 | `path`, `message` |
| `digest` | 선언과 docstring만 | `path`, `message` |
| `tree` | 파일 트리 | `path` |

각 도구는 CLI가 출력하는 것과 똑같은 내용을 돌려줍니다. 도구마다 일반 명령을 `-s --no-copy`로 실행하므로, 에이전트는 내가 붙여 넣을 내용과 똑같은 것을 받고 클립보드는 쓰이지 않습니다. [출력 형식](/ko/output-formats)을 참고하세요.

### 인자

- `targets`: 파일이나 폴더의 목록. `pack`에서는 이것들을 먼저 넣고, 파일을 지정하면 명령줄에서처럼 자동 항목이 꺼집니다.
- `with`: `pack`용. `failure`, `changes`, `tests`, `imports` 중 원하는 것의 목록. 기본값은 모두 켜기이고, `targets`를 주면 모두 끄기입니다.
- `budget`: `pack`용. 어림한 토큰 예산. 기본값 30000, 최솟값 1000.
- `message`: 결과에 넣을 요청 문구.
- `resolve`: `scan`용. 대상이 import하는 파일을 따라갑니다.
- `path`: 프로젝트 폴더. 서버의 작업 폴더를 기준으로 한 경로입니다.
- `count`: `terminal_history`용. 최근 명령 몇 개를 가져올지. 기본값 1.
- `failed`: `terminal_history`용. 종료 코드가 0이 아닌 명령만. 기본값 false.
- `here`: `terminal_history`용. 이 프로젝트와 그 아래에서 실행한 명령만. 기본값 true.

MCP로 호출한 `scan`은 크기를 묻느라 멈추는 일이 없습니다.

## 제한 {#limits}

- 기준점을 옮기는 도구는 없습니다. 기준점은 `cdg mark`로 내가 잡을 때만 바뀝니다.
- 서버를 실행한 프로젝트 밖의 경로는 심볼릭 링크나 정션을 거치더라도 거부합니다. 이때 도구는 오류를 돌려줍니다.
- 오류는 프로토콜 오류가 아니라 오류 표시가 붙은 도구 결과로 돌아오므로, 에이전트가 이유를 읽을 수 있습니다.
- `terminal_history`는 `cdg cap`이나 셸 훅으로 만든 기록이 필요합니다. 기록이 전혀 없으면 그렇다는 메시지를 돌려줍니다. 다만 `here=false`와 `failed=false`로 요청하면 셸 히스토리에서 읽어 오며, 이때는 비밀정보를 가린 명령만 나옵니다.

## 프로토콜

서버는 stdio로 한 줄에 메시지 하나씩 주고받는 작은 JSON-RPC 루프입니다. `initialize`, `tools/list`, `tools/call`, `ping`에 응답하고 알림은 무시합니다. MCP SDK에 의존하지 않습니다.

도구 목록을 직접 확인해 보려면:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | cdg mcp
```

두 번째 응답에는 `terminal_history`, `pack`, `scan`, `diff`, `semdiff`, `digest`, `tree`가 이 순서로 들어 있습니다.

## GitHub 저장소 (Python 빌드)

`scan_github`를 쓰려면 Python 서버를 원격 접근을 명시해 시작합니다.

```sh
uvx --python 3.15 codigest mcp --allow-remote
```

이 기능이 들어간 릴리스가 나오기 전까지는 소스에만 있는 기능입니다. 기본 `cdg mcp`의 도구 목록과 로컬 프로젝트 경계는 그대로입니다. Rust 빌드는 이 옵션을 아직 구현하지 않았습니다. GitHub 스캔은 저장소의 코드를 실행하지 않습니다.

`scan_github`는 `repository`(OWNER/REPO 또는 github.com URL), 선택 항목 `ref`, `paths`(FILE:START-END 범위를 포함한 목록), `format`(xml, markdown, json)을 받습니다. 확정된 커밋과 빠진 파일을 담은 CLI 스냅샷을 돌려줍니다. 원격 digest, tree, diff, pack은 없습니다. 원격 하위 프로세스마다 제한 시간은 110초입니다.

공개 스캔은 GitHub 자격 증명을 쓰지 않습니다. 비공개 저장소에 운영자의 기존 GitHub 자격 증명을 쓰게 하려면 `--allow-remote --auth`를 함께 주고 시작합니다. 도구를 호출하는 쪽은 인증을 켜거나 이 시작 정책을 바꿀 수 없습니다.

ChatGPT 클라우드에서는 이 stdio 명령을 Secure MCP Tunnel로 연결합니다. 명령을 시작하는 것만으로 ChatGPT 플러그인이 등록되지는 않습니다. [ChatGPT 비공개 연결](/ko/chatgpt-mcp)을 보십시오.
