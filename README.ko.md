# Codigest

[![PyPI](https://img.shields.io/pypi/v/codigest)](https://pypi.org/project/codigest/) [![Tests](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml/badge.svg)](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**실패한 명령, 그 명령이 가리키는 코드, 내가 바꾼 내용을 LLM에 한 번에 붙여 넣게 해 줍니다.**

[English](README.md)

<p align="center">
  <img src="docs/demo/demo.gif" alt="cdg cap으로 실패하는 pytest를 실행하고, cdg pack이 실패 출력과 그 출력이 가리키는 테스트 파일, 바뀐 파일을 묶는 화면" width="760">
</p>

## 핵심 아이디어

테스트가 실패해서 LLM에게 도움을 청합니다. 오류를 복사하고, 테스트 파일을 복사하고, 고친 파일을
복사합니다. 그래도 모델은 파일 하나를 더 보여 달라고 합니다.

Codigest가 그 복사를 대신합니다. 명령은 두 개면 됩니다.

```text
cdg cap -- pytest     명령을 실행하고 출력을 기억합니다
cdg pack              그 출력에 나온 파일을 찾고, 바뀐 내용을 더해
                      전부 클립보드에 복사합니다
```

그다음 붙여 넣으면 끝입니다. 평소에는 이것이 전부이고, 아래 내용은 설치, 설명, 선택 기능입니다.

## 설치

Codigest는 명령줄 도구입니다. [uv](https://docs.astral.sh/uv/)로 설치하면 필요한 Python(3.15)도
프로젝트와 상관없이 알아서 받아 둡니다.

```bash
winget install --id astral-sh.uv                  # Windows (uv가 있으면 생략)
curl -LsSf https://astral.sh/uv/install.sh | sh   # macOS/Linux

uv tool install codigest
uv tool update-shell                              # 그다음 터미널 다시 열기
```

명령은 `codigest`이고 짧게 `cdg`로 씁니다.
설치 중 `lazy import`에서 문법 오류가 나면 `uv self update` 후 다시 시도하세요.

## 첫 pack 만들기

실패하는 테스트가 있는 프로젝트로 갑니다. 준비할 것은 없습니다.

**1. 테스트를 `cdg cap`으로 실행합니다.** 명령 앞에 `--`를 붙입니다. 출력은 `cdg cap` 없이 실행할
때와 똑같고, Codigest는 사본만 남깁니다.

```bash
cdg cap -- pytest -q        # npm test, cargo test, go test ./..., make 등 무엇이든
```

**2. `cdg pack`을 실행합니다.** 무엇을 왜 골랐는지 알려 줍니다.

```console
$ cdg pack
Packed ~370 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py  line 5 in failing output
  + prices.py       changed
📋 Copied pack to clipboard
```

이렇게 읽으면 됩니다: 실패한 명령, Git의 커밋 안 된 변경, 오류가 5번째 줄을 가리킨
`test_prices.py`, 내가 고친 `prices.py`.

**3. 채팅창에 붙여 넣고** 원하는 것을 적습니다. 요청을 pack에 넣을 수도 있습니다:
`cdg pack -m "할인 계산을 고쳐 줘"`.

<details>
<summary>실제로 복사된 내용</summary>

XML 블록 하나입니다. 명령과 출력, diff, 그리고 줄 번호와 포함 이유가 붙은 파일들이 들어 있습니다.
위 pack 그대로이며 작업 디렉터리 경로만 줄였습니다.

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

예산을 넘어 빠진 파일은 `<omitted>` 블록에 이름이 남으므로, 모델은 무엇을 못 보고 있는지 압니다.

</details>

## 선택 기능 두 가지

기본 흐름에 익숙해지면, 각각 수작업 한 단계씩을 없애 줍니다.

**`cdg cap` 입력 생략.** 셸에 한 번 훅을 걸면 모든 명령이 자동으로 기록됩니다.

```bash
cdg term install            # 되돌리려면 cdg term uninstall
```

**붙여 넣기 생략.** 코딩 에이전트를 MCP로 연결하면 pack을 직접 가져갑니다.

```bash
claude mcp add --scope user codigest -- cdg mcp      # Claude Code
```

다른 클라이언트(Cursor, VS Code, Claude Desktop): 명령 `cdg`, 인자 `["mcp"]`로 stdio 서버를
추가합니다. 자세한 내용은 [참고](#자동-캡처)에 있습니다.

## `pack`이 고르는 방법

기대한 파일이 빠졌거나 예상 못 한 파일이 들어왔을 때 보면 됩니다. `pack`은 이 순서로 토큰
예산(기본 30,000)을 채웁니다.

1. 직접 지정한 파일: `cdg pack src/prices.py`
2. 이 프로젝트에서 마지막으로 실패한 명령의 출력에 나온 파일:
   `File "x.py", line 7`(Python), `x.py:7`(pytest, gcc, rustc, eslint), `x.ts(7,3)`(tsc).
   같은 명령이 다시 통과하면 그 실패는 더 이상 쓰지 않습니다.
3. 바꾼 파일: Git의 커밋 안 된 파일과 추적 안 된 파일. `cdg mark`로 기준점을 잡았다면 그
   이후의 변경([아래](#어느-시점-이후-바뀐-것-보기) 참고).
4. 바뀐 파일을 import하는 테스트(`test_*.py`, `*_test.py`, `*.test.ts`, `*.spec.js` 등).
5. 위 파일들이 import하는 프로젝트 파일: Python import와 JS/TS 상대 경로 import
   (`./x`, `x.ts`를 가리키는 `./x.js`, `dir/index.ts`를 가리키는 `./dir`).

실패한 명령과 diff는 항상 들어가고, `.env`, `*.pem`, `*.key`, SSH 키는 절대 들어가지 않습니다.
토큰은 네 글자를 하나로 어림하므로 예산은 대략적인 값입니다.

```bash
cdg pack -b 8000            # 예산 줄이기
cdg pack -s > context.xml   # 복사 대신 출력
```

## LLM에 줄 수 있는 다른 것들

`pack`은 실패한 테스트용입니다. 나머지 경우는 하고 싶은 일별로 정리했습니다.

### 앞서 실행한 명령 다시 보기

마지막 실패뿐 아니라 기록된 모든 명령을 꺼낼 수 있습니다.

```bash
cdg term              # 마지막 명령과 출력
cdg term -n 3         # 최근 3개
cdg term -f           # 마지막 실패
cdg term --here -n 3  # 이 프로젝트에서 실행한 최근 3개
```

### 어느 시점 이후 바뀐 것 보기

시작 전에 기준점을 잡고, 나중에 그 뒤로 바뀐 것을 받습니다.

```bash
cdg mark              # 예: 리팩터링 전에
# ... 작업 ...
cdg diff              # 기준점 이후 텍스트 diff
cdg semdiff           # Python만: 어떤 함수와 클래스가 바뀌었는지
```

기준점은 `.codigest/`에 두는 별도 사본이며 Git 기록은 건드리지 않습니다. 기준점을 잡으면 `pack`도
`git diff` 대신 이것을 씁니다.

### 프로젝트 전체, 또는 구조만 주기

```bash
cdg scan                # 모든 파일을 한 블록으로 (30k 토큰을 넘으면 먼저 확인)
cdg scan src/app.py -r  # 파일 하나와 그 파일이 import하는 로컬 파일
cdg digest              # Python 시그니처와 docstring만, 훨씬 작음
cdg tree                # 파일 트리만
```

## 참고

- [전체 명령](#전체-명령)
- [터미널 캡처](#터미널-캡처)
- [MCP 도구](#mcp-도구)
- [설정](#설정)
- [개인정보와 안전](#개인정보와-안전)
- [동작 원리](#동작-원리)
- [개발](#개발)

### 전체 명령

컨텍스트를 만드는 명령은 모두 클립보드에 복사합니다. `-s` / `--stdout`은 대신 출력하고,
`--no-copy`는 복사를 건너뛰며, `-m` / `--message`는 지시를 덧붙입니다.

| 명령 | 주는 것 | 프로젝트에 쓰는 것 |
| --- | --- | --- |
| `pack` | 마지막 실패, 그것이 가리키는 파일, 바뀐 내용 | 없음 |
| `cap` | 명령을 실행하고 출력을 보관 | 없음 |
| `term` | 최근 캡처한 명령과 출력 | 없음 |
| `mark` | 기준점 설정 | `.codigest/` |
| `diff` | 기준점 이후 텍스트 diff | `.codigest/changes.diff` |
| `semdiff` | 기준점 이후 함수·클래스 단위 변경 (Python) | `.codigest/semdiff.txt` |
| `scan` | 프로젝트 전체 또는 지정 경로를 한 블록으로 | `.codigest/snapshot.txt` |
| `digest` | 본문 없이 시그니처와 docstring만 (Python) | `.codigest/digest.txt` |
| `tree` | 프로젝트 트리 | 없음 |
| `status` | 기준점 시점, 대기 중인 변경, 생성된 파일 | 없음 |
| `init` | 설정 파일과 기준점. [파일 필터](#설정)를 바꿀 때만 필요 | `.codigest/`, `.gitignore` |
| `clean` | `.codigest/` 삭제 (먼저 묻고, `-y`면 묻지 않음) | `.codigest/` 삭제 |
| `mcp` | 에이전트용 MCP 서버 | 없음 |

추가 옵션: `scan -l`은 줄 번호, `scan -a`는 설정 필터 무시, `scan -y`는 확인 생략(CI).
`scan`, `diff`, `semdiff`, `digest`의 `-r` / `--resolve`는 대상이 import하는 로컬 파일도 포함합니다.
`cdg <명령> --help`로 전부 볼 수 있습니다.

### 터미널 캡처

`cdg cap`은 명령 출력을 그대로 보여 주면서 명령줄, stdout/stderr, 종료 코드, 작업 디렉터리를
기록합니다. 모든 프로젝트의 캡처가 컴퓨터에 하나 있는 저장소에 쌓이므로, `cap`과 `term`은 어느
디렉터리에서나 동작합니다.

```bash
cdg cap "npm run build | tail -20"  # 파이프는 따옴표로
cdg term -i 2         # 두 번 전 명령 하나만
cdg term path         # 캡처 저장 위치 (CODIGEST_HOME으로 바꿀 수 있음)
cdg term clear        # 캡처 전부 삭제
```

`cap`을 거치지 않은 명령은 셸 자체 기록(PowerShell, bash, zsh, fish)에서 읽으며, 출력은 없습니다.

#### 자동 캡처

`cdg term install`은 설치된 셸에 따라 PowerShell 프로필, `~/.bashrc`, `~/.zshrc`를 수정하며
`--shell powershell|bash|zsh|all`로 고를 수 있습니다. Windows에서는 Git Bash가 bash로 취급됩니다.
명령은 현재 세션에서 그대로 실행되고(`cd`, 변수, 함수 모두 정상) 기록에도 입력한 그대로 남습니다.
편집기, 페이저, REPL, `ssh`는 건너뜁니다.

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

### MCP 도구

| 도구 | 돌려주는 것 |
| --- | --- |
| `pack` | 마지막 실패, 그것이 가리키는 파일, 바뀐 내용과 import, 예산에 맞춰 (`targets`, `budget`, `message`) |
| `terminal_history` | 최근 명령과 출력, 종료 코드 (`count`, `failed`, `here`: 이 프로젝트만, 기본값 켜짐) |
| `scan` | 프로젝트 스냅샷 (`targets`, `resolve`, `message`) |
| `diff` / `semdiff` | 기준점 이후 텍스트 / 구조 변경 |
| `digest` | 시그니처와 docstring만 |
| `tree` | 디렉터리 트리 |

도구는 CLI가 출력하는 것과 같은 텍스트를 돌려줍니다. 어떤 도구도 기준점을 옮기지 않으며, 서버가
실행된 프로젝트 밖의 경로는 거부합니다.

### 설정

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

### 개인정보와 안전

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

### 동작 원리

- **기준점.** `.codigest/anchor/.shadow_git`에 있는 Git 저장소로, 프로젝트 저장소와 분리되어
  있습니다. 편집기가 프로젝트 저장소로 착각하지 않도록 이름이 `.git`이 아닙니다. `mark`와
  `init`이 현재 상태를 커밋하고, `diff`, `semdiff`, `status`, `pack`이 작업 트리와 비교합니다.
- **캡처.** 줄 단위 JSON 파일 하나에 최근 명령 200개를 둡니다. 출력은 명령마다 64 KB까지이며
  앞부분과 뒷부분을 남깁니다.
- **출력.** 파일 내용은 `<file>` 블록 안에서 XML 이스케이프되고, 블록은 왼쪽 끝에 붙여 써서
  불필요한 들여쓰기가 프롬프트에 섞이지 않습니다.
- **MCP.** 서버는 stdio 위의 작은 JSON-RPC 루프입니다. 각 도구는 일반 CLI를 `-s --no-copy`로
  실행하므로, 에이전트는 내가 붙여 넣을 것과 똑같은 내용을 봅니다.

### 개발

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
