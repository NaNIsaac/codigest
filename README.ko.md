# Codigest

[![PyPI](https://img.shields.io/pypi/v/codigest)](https://pypi.org/project/codigest/) [![Tests](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml/badge.svg)](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**실패한 명령, 그 명령이 가리키는 코드, 내가 바꾼 내용을 LLM에 한 번에 붙여 넣게 해 줍니다.**

[English](README.md)

<p align="center">
  <img src="docs/demo/demo.gif" alt="cdg cap으로 실패하는 pytest를 실행하고, cdg pack이 실패 출력과 그 출력이 가리키는 테스트 파일, 바뀐 파일을 묶는 화면" width="760">
</p>

테스트가 실패해서 ChatGPT, Claude, 코딩 에이전트에게 도움을 청하면 보통 오류를 복사하고, 테스트
파일을 복사하고, 고친 파일을 복사합니다. 그래도 모델은 파일 하나를 더 보여 달라고 합니다.
Codigest는 이 수집을 대신합니다.

1. `cdg cap -- pytest`는 명령을 평소처럼 실행하고 출력을 기억합니다.
2. `cdg pack`은 그 출력에서 프로젝트 파일 이름을 찾고, 바뀐 내용을 더해 하나의 블록으로
   클립보드에 복사합니다.
3. 채팅창에 붙여 넣으면 됩니다. MCP로 연결된 에이전트는 같은 블록을 직접 가져갑니다.

모든 작업은 내 컴퓨터에서 이루어집니다. Codigest는 파일과 Git 상태를 읽기만 하고, 소스 파일이나
Git 기록을 바꾸지 않습니다.

## 목차

- [설치](#설치)
- [빠르게 시작하기](#빠르게-시작하기)
- [LLM이 받는 내용](#llm이-받는-내용)
- [`pack`이 파일을 고르는 방법](#pack이-파일을-고르는-방법)
- [용어](#용어)
- [명령어](#명령어)
- [터미널 캡처](#터미널-캡처)
- [에이전트에서 쓰기 (MCP)](#에이전트에서-쓰기-mcp)
- [설정](#설정)
- [개인정보와 안전](#개인정보와-안전)
- [동작 원리](#동작-원리)
- [개발](#개발)

## 설치

Codigest는 Python으로 만든 명령줄 도구입니다. Python 3.15가 필요하지만 직접 설치할 필요는
없습니다. [uv](https://docs.astral.sh/uv/)가 도구 전용으로 받아 프로젝트와 분리해 둡니다.
프로젝트 의존성에 넣는 패키지가 아닙니다.

```bash
# 1. uv가 없다면 설치
winget install --id astral-sh.uv                    # Windows
curl -LsSf https://astral.sh/uv/install.sh | sh     # macOS/Linux

# 2. codigest 설치
uv tool install codigest

# 3. uv 도구 경로를 PATH에 추가하고 터미널을 다시 열기
uv tool update-shell
```

명령은 `codigest`이고 짧게 `cdg`로도 씁니다. 설치 없이 써 보려면 `uvx codigest tree`.

`uv`가 `lazy import`에서 문법 오류를 내면 초기 3.15 알파를 고른 것입니다. `uv self update` 후
다시 설치하세요.

## 빠르게 시작하기

준비할 것은 없습니다. 아무 프로젝트에서 테스트를 `cdg cap`으로 실행합니다.

```bash
cd my-project
cdg cap -- pytest -q        # npm test, cargo test, go test ./..., make 등 무엇이든
```

출력은 `cdg cap` 없이 실행할 때와 똑같이 보입니다. 실패했다면 pack을 만듭니다.

```bash
cdg pack
```

`cdg pack`은 무엇을 골랐는지 보여 주고 블록을 클립보드에 복사합니다.

```console
$ cdg pack
Packed ~370 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py  line 5 in failing output
  + prices.py       changed
📋 Copied pack to clipboard
```

채팅창에 붙여 넣고 원하는 것을 적거나, 요청을 pack에 함께 넣을 수 있습니다:
`cdg pack -m "할인 계산을 고쳐 줘"`.

더 편하게 쓰려면 두 가지를 해 두세요.

- `cdg term install`: 셸에 훅을 걸어 `cdg cap` 없이도 모든 명령을 기록합니다.
  [터미널 캡처](#터미널-캡처) 참고.
- `claude mcp add --scope user codigest -- cdg mcp`: Claude Code(또는 다른 MCP 클라이언트)가
  클립보드 없이 `pack`을 직접 호출합니다. [에이전트에서 쓰기](#에이전트에서-쓰기-mcp) 참고.

## LLM이 받는 내용

클립보드에는 XML 블록 하나가 들어갑니다. 위 데모에서 만든 pack이며, 작업 디렉터리 경로만
줄였습니다.

```xml
<context_pack project="project">
<terminal_history>
<command index="1" command="pytest -q" exit_code="1" cwd="~/project" duration="1.5s" at="2026-10-17T01:58:14+09:00">
<output>
F                                                                      [100%]
================================= FAILURES ==================================
_____________________________ test_ten_percent ______________________________

    def test_ten_percent():
&gt;       assert apply_discount(200, 10) == 180
E       assert -1800 == 180
E        +  where -1800 = apply_discount(200, 10)

test_prices.py:5: AssertionError
========================== short test summary info ==========================
FAILED test_prices.py::test_ten_percent - assert -1800 == 180
1 failed in 0.11s
</output>
</command>
</terminal_history>
<changes source="uncommitted changes (git)">
diff --git a/prices.py b/prices.py
index 3d691df..428752a 100644
--- a/prices.py
+++ b/prices.py
@@ -1,2 +1,2 @@
 def apply_discount(price, percent):
-    return price * (1 - percent / 100)
+    return price * (1 - percent)
</changes>
<file path="test_prices.py" reason="line 5 in failing output">
1: from prices import apply_discount
2: 
3: 
4: def test_ten_percent():
5:     assert apply_discount(200, 10) == 180
</file>
<file path="prices.py" reason="changed">
1: def apply_discount(price, percent):
2:     return price * (1 - percent)
</file>
</context_pack>
```

파일에는 줄 번호가 붙어 있어 모델이 `test_prices.py:5`를 바로 찾을 수 있습니다. 파일마다 포함된
이유가 적혀 있습니다. 예산을 넘어 빠진 파일은 `<omitted>` 블록에 이름이 남으므로, 모델은 무엇을
못 보고 있는지 알고 요청할 수 있습니다.

## `pack`이 파일을 고르는 방법

토큰 예산(기본 30,000)이 찰 때까지 다음 순서로 파일을 넣습니다.

1. 직접 지정한 파일: `cdg pack src/prices.py`
2. 이 프로젝트에서 마지막으로 실패한 명령의 출력에 나온 파일:
   `File "x.py", line 7`(Python), `x.py:7`(pytest, gcc, rustc, eslint), `x.ts(7,3)`(tsc).
   같은 명령이 이후 통과하면 그 실패는 빠집니다. 고친 테스트가 다시 딸려 오지 않습니다.
3. 바뀐 파일: `cdg mark`로 기준점을 잡았다면 그 이후, 아니면 Git의 커밋 안 된 파일과 추적 안 된 파일.
4. 바뀐 Python 파일을 import하는 테스트(`test_*.py`, `*_test.py`). 코드가 어떻게 동작해야 하는지
   모델이 볼 수 있습니다.
5. 위 파일들이 import하는 프로젝트 내 Python 모듈.

실패한 명령과 diff는 항상 들어갑니다. `.env`, `*.pem`, `*.key`, `id_rsa` 같은 이름의 파일은 절대
넣지 않습니다. 토큰 수는 네 글자를 한 토큰으로 어림하므로 예산은 대략적인 값입니다.

```bash
cdg pack                           # 여기서 마지막 실패 + 바뀐 내용
cdg pack -m "할인 버그 고쳐 줘"      # 지시 추가
cdg pack src/prices.py -b 8000     # 파일 추가, 예산 축소
cdg pack -s > context.xml          # 복사 대신 출력
```

실패한 명령도 바뀐 것도 없으면 `pack`은 그렇다고 알려 주고 다음에 실행할 것을 제안합니다.

## 용어

이 문서에서 계속 나오는 세 단어입니다.

- **캡처(capture).** `cdg cap`(또는 셸 훅)으로 실행한 명령과 그 출력, 종료 코드, 작업 디렉터리.
  캡처는 프로젝트 안이 아니라 컴퓨터 전체에서 하나의 저장소에 쌓이므로 어느 디렉터리에서든
  동작합니다.
- **기준점(baseline).** `diff`, `semdiff`, `pack`이 비교 대상으로 삼는 프로젝트 파일의 사본입니다.
  `cdg mark`로 잡습니다. 기준점이 없으면 `pack`은 Git의 커밋 안 된 변경을 대신 씁니다.
  대부분은 이쪽을 원합니다.
- **컨텍스트(context).** 명령이 LLM용으로 만들어 내는 XML 텍스트입니다. 컨텍스트를 만드는
  명령은 모두 클립보드에 복사합니다. `-s`는 대신 출력하고, `--no-copy`는 복사를 건너뜁니다.

## 명령어

| 명령 | 주는 것 | 프로젝트에 쓰는 것 |
| --- | --- | --- |
| [`pack`](#pack이-파일을-고르는-방법) | 마지막 실패, 그것이 가리키는 파일, 바뀐 내용 | 없음 |
| [`cap`](#터미널-캡처) | 명령을 실행하고 출력을 보관 | 없음 |
| [`term`](#터미널-캡처) | 최근 캡처한 명령과 출력 | 없음 |
| [`mark`](#mark) | 기준점 설정 | `.codigest/` |
| [`diff`](#diff) | 기준점 이후 텍스트 diff | `.codigest/changes.diff` |
| [`semdiff`](#semdiff) | 기준점 이후 함수·클래스 단위 변경 (Python) | `.codigest/semdiff.txt` |
| [`scan`](#scan) | 프로젝트 전체 또는 지정 경로를 한 블록으로 | `.codigest/snapshot.txt` |
| [`digest`](#digest) | 본문 없이 시그니처와 docstring만 (Python) | `.codigest/digest.txt` |
| [`tree`](#tree) | 프로젝트 트리 | 없음 |
| [`status`](#status) | 기준점 시점, 대기 중인 변경, 생성된 파일 | 없음 |
| [`init`](#init) | 설정 파일과 기준점 | `.codigest/`, `.gitignore` |
| [`clean`](#clean) | `.codigest/` 삭제 | `.codigest/` 삭제 |
| [`mcp`](#에이전트에서-쓰기-mcp) | 에이전트용 MCP 서버 | 없음 |

공통 옵션: `-s` / `--stdout`은 저장 대신 출력, `--no-copy`는 클립보드 생략, `-m` / `--message`는
지시 추가, `-r` / `--resolve`(`scan`, `diff`, `semdiff`, `digest`)는 대상이 import하는 로컬 파일도
포함합니다.

### `mark`

현재 상태를 기준점으로 기록합니다. 다시 실행하면 기준점이 옮겨집니다.

```bash
cdg mark                # 예: 리팩터링 전에
# ... 수정 ...
cdg diff                # 그 뒤로 바뀐 모든 것
```

### `diff`

작업 트리와 기준점의 텍스트 diff입니다. 실제 Git 저장소는 건드리지 않습니다.

```bash
cdg diff
cdg diff -s
```

### `semdiff`

Python 파일의 구조 diff입니다. 함수, 메서드, 클래스, 전역 변수의 추가·삭제·수정과 시그니처
변경, 본문 변경을 보여 줍니다. 포맷과 주석 변경은 무시합니다.

```bash
cdg semdiff
```

### `scan`

프로젝트 트리와 파일 내용을 한 블록으로 만듭니다. 먼저 크기를 어림하고 30k 토큰이나 파일
100개를 넘으면 확인을 받습니다. `scan`은 기준점을 옮기지 않습니다.

```bash
cdg scan                                   # 프로젝트 전체
cdg scan src/main.py -r                    # 파일 하나와 그 파일이 import하는 로컬 파일
cdg scan -y --message "Automated snapshot" # 확인 없이 (CI)
cdg scan -s                                # 저장 대신 출력
```

그 밖의 옵션: `-l`은 줄 번호 추가, `-a`는 설정 필터 무시, `-o`는 출력 파일 이름 지정.

### `digest`

프로젝트 트리와 Python 파일의 시그니처, docstring만 담고 함수 본문은 뺍니다. 큰 프로젝트의
모양을 적은 토큰으로 보여 줄 때 씁니다.

```bash
cdg digest
```

### `tree`

`.gitignore`와 설정 필터를 따르는 프로젝트 트리를 출력합니다.

### `status`

기준점이 언제 잡혔는지, 그 뒤로 바뀐 파일, `.codigest/`에 생성된 파일을 보여 줍니다.

### `init`

`.codigest/config.toml`을 만들고([설정](#설정) 참고), `mark`처럼 기준점을 잡고, `.codigest/`를
`.gitignore`에 추가합니다. 파일 필터를 바꾸고 싶을 때만 필요합니다.

### `clean`

기준점과 생성된 파일을 포함해 `.codigest/`를 지웁니다. 먼저 묻고, `-y`면 묻지 않습니다.

## 터미널 캡처

`cdg cap`은 명령을 실행하면서 출력을 그대로 보여 주고, 명령줄과 stdout/stderr, 종료 코드, 작업
디렉터리를 기록합니다. `cdg term`은 최근 캡처를 `<terminal_history>` 블록으로 돌려줍니다. 둘 다
`cdg init`이 필요 없습니다.

```bash
cdg cap -- pytest -q                # 명령 앞에 --를 붙여야 그 명령의 옵션이 전달됩니다
cdg cap "npm run build | tail -20"  # 파이프는 따옴표로

cdg term              # 마지막 명령
cdg term -n 3         # 최근 3개
cdg term -i 2         # 두 번 전 명령 하나만
cdg term -f           # 마지막 실패 명령
cdg term --here -n 3  # 이 프로젝트에서 실행한 최근 3개

cdg term path         # 캡처 저장 위치 (CODIGEST_HOME으로 바꿀 수 있음)
cdg term clear        # 캡처 전부 삭제
```

`cap`을 거치지 않은 명령은 셸 자체 기록(PowerShell, bash, zsh, fish)에서 읽으며, 출력은 없습니다.

### 자동 캡처

`cdg term install`은 셸에 훅을 걸어 `cap` 없이도 모든 명령을 기록합니다. 설치된 셸에 따라
PowerShell 프로필, `~/.bashrc`, `~/.zshrc`를 수정하며 `--shell powershell|bash|zsh|all`로 고를 수
있습니다. Windows에서는 Git Bash가 bash로 취급됩니다. 명령은 현재 세션에서 그대로 실행되고
(`cd`, 변수, 함수 모두 정상) 기록에도 입력한 그대로 남습니다. 편집기, 페이저, REPL, `ssh`는
건너뜁니다.

- 명령 하나만 건너뛰기: 줄을 공백으로 시작합니다.
- 다른 프로그램도 건너뛰기: `$env:CODIGEST_CAP_SKIP = '^\s*(myrepl|mytui)\b'` (bash/zsh:
  `export CODIGEST_CAP_SKIP='^(myrepl|mytui)( |$)'`, 확장 정규식).
- 이번 세션만 끄기: `$env:CODIGEST_NOCAP = 1` (bash/zsh: `export CODIGEST_NOCAP=1`).
- 훅 제거: `cdg term uninstall`.

제약:

- 캡처되는 프로그램은 TTY가 아니라 파이프에 쓰므로 일부는 색이나 진행 막대가 사라집니다.
- bash(4 이상)와 zsh에서 여러 줄 명령(여러 줄로 입력한 반복문, heredoc), 백그라운드 작업
  (`cmd &`), `!` 기록 확장을 쓰는 줄은 기록되지 않고 그냥 실행됩니다. bash에서는 서브셸
  `( ... )`로 시작하는 줄도 마찬가지입니다.
- bash 훅은 DEBUG 트랩을 쓰며, 첫 프롬프트 전에 설정된 트랩(starship, bash-preexec)은
  유지합니다. 그 뒤에 설정된 DEBUG 트랩은 훅을 대체합니다.

## 에이전트에서 쓰기 (MCP)

`cdg mcp`는 Codigest를 stdio [MCP](https://modelcontextprotocol.io) 서버로 띄웁니다. 코딩
에이전트가 붙여 넣기를 부탁하는 대신 직접 컨텍스트를 가져갑니다.

```bash
# Claude Code
claude mcp add --scope user codigest -- cdg mcp
```

다른 클라이언트(Cursor, VS Code, Claude Desktop): 명령 `cdg`, 인자 `["mcp"]`로 stdio 서버를
추가합니다.

| 도구 | 돌려주는 것 |
| --- | --- |
| `pack` | 마지막 실패, 그것이 가리키는 파일, 바뀐 내용과 import, 예산에 맞춰 (`targets`, `budget`, `message`) |
| `terminal_history` | 최근 명령과 출력, 종료 코드 (`count`, `failed`, `here`: 이 프로젝트만, 기본값 켜짐) |
| `scan` | 프로젝트 스냅샷 (`targets`, `resolve`, `message`) |
| `diff` / `semdiff` | 기준점 이후 텍스트 / 구조 변경 |
| `digest` | 시그니처와 docstring만 |
| `tree` | 디렉터리 트리 |

도구는 CLI가 출력하는 것과 같은 텍스트를 돌려줍니다. 어떤 도구도 기준점을 옮기지 않으며,
서버가 실행된 프로젝트 밖의 경로는 거부합니다.

## 설정

`cdg init`은 `.codigest/config.toml`을 만듭니다. `[filter]` 절이 모든 명령이 다룰 파일을 정합니다.

```toml
[filter]
# 이 확장자만 (생략하면 무시되지 않은 모든 파일)
extensions = [".py", ".ts", ".rs", ".md", ".json"]

# 추가 제외, .gitignore 문법 (.gitignore 자체는 항상 따름)
exclude_patterns = [
    "*.lock",
    "tests/data/",
    "legacy_code/"
]
```

`.git/`, `.codigest/`, 가상환경, `node_modules/`, `__pycache__/`는 항상 건너뜁니다.

## 개인정보와 안전

- **캡처 속 비밀정보.** 알려진 비밀정보 형태(API 키, 토큰, `password=...`, 개인 키, URL 속 인증
  정보)는 디스크에 쓰기 전에 가립니다. 인증정보를 출력하는 명령(`env`, `printenv`,
  `gh auth token`, `aws configure` 등)의 출력은 아예 저장하지 않습니다. 흔한 형식을 잡는 것이지
  모든 비밀정보를 잡는 것은 아닙니다.
- **비밀정보 파일.** `pack`은 `.env*`, `*.pem`, `*.key`, `*.p12`, `*.pfx`, SSH 개인 키를 절대 넣지
  않고 제외 목록에 남깁니다.
- **내 프로젝트.** 어떤 명령도 소스 파일을 바꾸지 않습니다. 저장되는 출력과 기준점은
  `.codigest/` 안에만 있고, `init`이 이 디렉터리를 `.gitignore`에 넣습니다. 기준점이 없으면
  `pack`은 아무것도 만들지 않습니다.
- **캡처 위치.** `cdg term path`가 파일 위치를 보여 줍니다. macOS와 Linux에서는 나만 읽을 수
  있습니다. `cdg term clear`로 지웁니다.
- **밖으로 나가는 것은 없습니다.** Codigest는 네트워크 요청을 하지 않습니다. 컨텍스트는 내가
  붙여 넣은 곳이나 연결한 MCP 클라이언트로만 갑니다.

## 동작 원리

- **기준점.** 기준점은 `.codigest/anchor/.shadow_git`에 있는 Git 저장소로, 프로젝트 저장소와
  분리되어 있습니다. 편집기가 프로젝트 저장소로 착각하지 않도록 이름이 `.git`이 아닙니다.
  `mark`와 `init`이 현재 상태를 커밋하고, `diff`, `semdiff`, `status`, `pack`이 작업 트리와
  비교합니다.
- **캡처.** 줄 단위 JSON 파일 하나에 최근 명령 200개를 둡니다. 출력은 명령마다 64 KB까지이며
  앞부분과 뒷부분을 남깁니다.
- **출력.** 파일 내용은 `<file>` 블록 안에서 XML 이스케이프되고, 블록은 왼쪽 끝에 붙여 써서
  불필요한 들여쓰기가 프롬프트에 섞이지 않습니다.
- **MCP.** 서버는 stdio 위의 작은 JSON-RPC 루프입니다. 각 도구는 일반 CLI를 `-s --no-copy`로
  실행하므로, 에이전트는 내가 붙여 넣을 것과 똑같은 내용을 봅니다.

## 개발

```bash
git clone https://github.com/NaNIsaac/codigest
cd codigest
uv run cdg --help              # 체크아웃에서 실행
uv run --with pytest pytest -q # 테스트
```

데모 GIF는 기록한 출력을 [`docs/demo/make_gif.py`](docs/demo/make_gif.py)가 그려서 만듭니다.
새로 기록하는 방법은 그 파일의 docstring에 있습니다.

## 라이선스

MIT. [LICENSE](LICENSE) 참고.
