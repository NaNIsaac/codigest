# Codigest

[![PyPI](https://img.shields.io/pypi/v/codigest)](https://pypi.org/project/codigest/) [![Tests](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml/badge.svg)](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**AI에게 코드 질문을 할 때 함께 보낼 자료를 터미널과 프로젝트에서 골라 주는 도구**

[English](README.md)

Codigest는 명령줄 도구입니다. ChatGPT나 Claude, 코딩 에이전트에게 실패한 테스트나 작업 중인
코드를 물어볼 때, 모델이 알아야 할 내용을 대신 모아 줍니다.

모델은 내가 보여 준 만큼만 압니다. 제대로 답하려면 오류 메시지와 관련 코드, 내가 고친 부분이
있어야 합니다. 그런데 이걸 손으로 모으다 보면 정작 중요한 파일 하나를 빼먹거나, 반대로 상관없는
코드까지 잔뜩 붙여 넣게 됩니다.

Codigest는 내가 실행한 명령과 그 출력을 기억해 두었다가, 오류가 가리키는 파일과 Git에서 바뀐
부분을 찾아 하나의 텍스트로 묶어 줍니다. 채팅창에 붙여 넣어도 되고, 에이전트가 직접 가져가게 할
수도 있습니다.

## 하는 일

- **터미널 출력을 기억합니다.** 명령과 출력, 종료 코드를 저장합니다. 토큰이나 비밀번호처럼 보이는
  값은 가린 뒤에 저장합니다.
- **관련 있는 코드를 고릅니다.** 오류에 나온 파일과 줄, Git에서 바뀐 파일을 찾고, 그 파일을 쓰는
  테스트와 import까지 따라갑니다.
- **고른 이유를 밝힙니다.** 파일마다 왜 넣었는지 적혀 있고, 분량 때문에 뺀 파일도 이름은 남깁니다.
- **분량을 맞춥니다.** 큰 파일은 오류 근처만 잘라 넣어 토큰 예산에 맞춥니다.
- **에이전트와 연결됩니다.** Claude Code, Cursor 같은 MCP 클라이언트가 같은 결과를 직접 받아 갑니다.

Codigest는 AI 모델을 호출하지 않습니다. 모델에게 보낼 자료를 준비할 뿐입니다. 모든 작업은 내
컴퓨터 안에서 이루어지고, 소스 파일과 Git 기록은 읽기만 합니다.

<p align="center">
  <img src="docs/demo/demo.gif" alt="cdg cap으로 실패하는 pytest를 실행한 뒤, cdg pack이 실패 출력과 오류가 가리키는 테스트 파일, 바뀐 파일을 묶는 화면" width="760">
  <br>
  <sub>테스트가 실패한 뒤, 명령 하나로 모델에게 보낼 자료를 모으는 모습</sub>
</p>

## 설치

[uv](https://docs.astral.sh/uv/)로 설치합니다. uv가 필요한 Python(3.15)까지 알아서 받아 두므로,
내 프로젝트의 Python 환경과 섞이지 않습니다.

```bash
winget install --id astral-sh.uv                  # Windows (uv가 이미 있으면 건너뛰기)
curl -LsSf https://astral.sh/uv/install.sh | sh   # macOS/Linux

uv tool install codigest
uv tool update-shell                              # 끝나면 터미널을 다시 열기
```

명령 이름은 `codigest`이고, 줄여서 `cdg`로도 쓸 수 있습니다.
설치할 때 `lazy import` 부분에서 문법 오류가 나면 `uv self update`를 한 뒤 다시 설치하세요.

## 처음 써 보기

테스트가 실패하는 프로젝트에서 바로 해 볼 수 있습니다. 미리 설정할 것은 없습니다.

**1. 테스트를 `cdg cap`으로 실행합니다.** 실행할 명령 앞에 `--`를 붙입니다. 화면에는 평소와 똑같이
출력되고, Codigest는 그 내용을 따로 저장해 둡니다.

```bash
cdg cap -- pytest -q        # npm test, cargo test, go test ./..., make 무엇이든 됩니다
```

**2. `cdg pack`을 실행합니다.** 무엇을 왜 골랐는지 보여 주고, 결과를 클립보드에 복사합니다.

```console
$ cdg pack
Packed ~370 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py  line 5 in failing output
  + prices.py       changed
📋 Copied pack to clipboard
```

위에서부터 실패한 명령, Git에서 아직 커밋하지 않은 변경, 오류가 5번째 줄을 가리킨
`test_prices.py`, 내가 고친 `prices.py`를 담았다는 뜻입니다.

**3. 채팅창에 붙여 넣고** 원하는 것을 적습니다. 요청을 미리 넣어 둘 수도 있습니다:
`cdg pack -m "할인 계산이 틀린 이유를 찾아서 고쳐 줘"`.

<details>
<summary>클립보드에 실제로 들어간 내용</summary>

XML 블록 하나입니다. 명령과 출력, diff, 그리고 줄 번호와 포함한 이유가 붙은 파일이 들어 있습니다.
위 예시 그대로이고 작업 경로만 줄였습니다.

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

분량이 넘쳐 뺀 파일은 `<omitted>` 블록에 이름이 남기 때문에, 모델도 무엇이 빠졌는지 알 수 있습니다.

</details>

## 더 편하게 쓰기

기본 흐름에 익숙해졌다면, 아래 두 가지로 손이 한 번씩 덜 갑니다.

**`cdg cap`을 매번 붙이지 않기.** 셸에 한 번 훅을 설치하면 모든 명령이 자동으로 기록됩니다.

```bash
cdg term install            # 되돌릴 때는 cdg term uninstall
```

**붙여 넣지 않기.** 코딩 에이전트를 MCP로 연결하면 에이전트가 직접 가져갑니다.

```bash
claude mcp add --scope user codigest -- cdg mcp      # Claude Code
```

Cursor, VS Code, Claude Desktop 같은 다른 클라이언트에서는 명령 `cdg`, 인자 `["mcp"]`로 stdio
서버를 추가하면 됩니다. 자세한 내용은 [참고 자료](#자동-기록)에 있습니다.

## `pack`이 파일을 고르는 기준

넣을 줄 알았던 파일이 빠졌거나 뜻밖의 파일이 들어왔을 때 확인하세요. `pack`은 아래 순서대로
토큰 예산(기본 30,000)을 채웁니다.

1. 직접 지정한 파일: `cdg pack src/prices.py`
2. 이 프로젝트에서 최근 한 시간 안에 실패한 마지막 명령의 출력에 나온 파일.
   `File "x.py", line 7`(Python), `x.py:7`(pytest, ruff, gcc, rustc, eslint), `x.ts(7,3)`(tsc)
   같은 형식을 알아봅니다. 같은 명령이 나중에 성공하면 그 실패는 더 이상 쓰지 않습니다.
   내가 고치지 않았는데 출력에만 나온 파일은, 오류가 가리킨 곳마다 앞뒤 열 줄씩만 잘라서 넣고
   `lines="40-60"`처럼 표시합니다. 짧은 파일은 전체를 넣습니다.
3. 내가 바꾼 파일: Git에서 아직 커밋하지 않은 파일과 새로 만든 파일. `cdg mark`로 기준점을
   잡아 두었다면 그 뒤로 바뀐 파일([아래](#특정-시점-이후의-변경-사항-보기) 참고).
4. 바뀐 파일을 import하는 테스트(`test_*.py`, `*_test.py`, `*.test.ts`, `*.spec.js` 등).
5. 1·3·4번 파일이 import하는 프로젝트 안의 파일. Python import와 JS/TS 상대 경로 import를
   따라갑니다(`./x`, `x.ts`를 가리키는 `./x.js`, `dir/index.ts`를 가리키는 `./dir`).

실패한 명령과 diff는 항상 넣고, `.env`, `*.pem`, `*.key`, SSH 키는 절대 넣지 않습니다. 토큰 수는
네 글자를 한 토큰으로 계산한 어림값이라 실제와 조금 다를 수 있습니다.

```bash
cdg pack -b 8000            # 예산을 줄이기
cdg pack -s > context.xml   # 클립보드 대신 화면에 출력
```

## 다른 상황에서 쓰기

`pack`은 테스트가 실패했을 때를 위한 명령입니다. 다른 상황에서는 아래 명령을 씁니다.

### 이전에 실행한 명령 꺼내기

마지막 실패뿐 아니라 기록해 둔 명령은 모두 다시 꺼낼 수 있습니다.

```bash
cdg term              # 마지막 명령과 출력
cdg term -n 3         # 최근 3개
cdg term -f           # 마지막으로 실패한 명령
cdg term --here -n 3  # 이 프로젝트에서 실행한 최근 3개
```

### 특정 시점 이후의 변경 사항 보기

작업을 시작하기 전에 기준점을 잡아 두면, 나중에 그 뒤로 바뀐 내용을 한꺼번에 받을 수 있습니다.

```bash
cdg mark              # 예: 리팩터링을 시작하기 전에
# ... 작업 ...
cdg diff              # 기준점 이후의 diff
cdg semdiff           # Python 전용: 어떤 함수와 클래스가 바뀌었는지
```

기준점은 `.codigest/` 폴더에 따로 저장하는 복사본이라 Git 기록에는 영향이 없습니다. 기준점을 잡아
두면 `pack`도 `git diff` 대신 이것과 비교합니다.

### 프로젝트 전체나 구조만 넘기기

```bash
cdg scan                # 모든 파일을 하나로 묶기 (3만 토큰이 넘으면 먼저 확인)
cdg scan src/app.py -r  # 파일 하나와, 그 파일이 import하는 프로젝트 파일
cdg digest              # Python 함수 선언과 docstring만, 훨씬 짧음
cdg tree                # 파일 트리만
```

## 참고 자료

- [전체 명령](#전체-명령)
- [터미널 기록](#터미널-기록)
- [MCP 도구](#mcp-도구)
- [설정](#설정)
- [개인정보와 안전](#개인정보와-안전)
- [동작 방식](#동작-방식)
- [개발](#개발)

### 전체 명령

결과를 만드는 명령은 모두 클립보드에 복사합니다. `-s` / `--stdout`을 붙이면 화면에 출력하고,
`--no-copy`를 붙이면 복사하지 않으며, `-m` / `--message`로 요청 문구를 덧붙일 수 있습니다.

| 명령 | 하는 일 | 프로젝트에 만드는 파일 |
| --- | --- | --- |
| `pack` | 마지막 실패와 관련 파일, 변경 사항 묶기 | 없음 |
| `cap` | 명령을 실행하고 출력 기록 | 없음 |
| `term` | 최근 기록한 명령과 출력 보기 | 없음 |
| `mark` | 기준점 잡기 | `.codigest/` |
| `diff` | 기준점 이후의 diff | `.codigest/changes.diff` |
| `semdiff` | 기준점 이후 함수·클래스 단위 변경 (Python) | `.codigest/semdiff.txt` |
| `scan` | 프로젝트 전체나 지정한 경로를 하나로 묶기 | `.codigest/snapshot.txt` |
| `digest` | 함수 본문을 뺀 선언과 docstring (Python) | `.codigest/digest.txt` |
| `tree` | 프로젝트 파일 트리 | 없음 |
| `status` | 지금 `pack`이 쓸 내용 확인: 마지막 실패, 변경 사항, 기준점 | 없음 |
| `init` | 설정 파일 만들기와 기준점 잡기. [파일 필터](#설정)를 바꿀 때만 필요 | `.codigest/`, `.gitignore` |
| `clean` | `.codigest/` 지우기 (먼저 물어보며, `-y`면 묻지 않음) | `.codigest/` 삭제 |
| `mcp` | 에이전트용 MCP 서버 실행 | 없음 |

그 밖의 옵션: `scan -l`은 줄 번호를 붙이고, `scan -a`는 설정 필터를 무시하며, `scan -y`는 확인 없이
진행합니다(CI용). `scan`, `diff`, `semdiff`, `digest`에 `-r` / `--resolve`를 붙이면 대상 파일이
import하는 프로젝트 파일도 함께 넣습니다. `cdg <명령> --help`로 모든 옵션을 볼 수 있습니다.

### 터미널 기록

`cdg cap`은 명령의 출력을 그대로 보여 주면서 명령어, 출력(stdout과 stderr), 종료 코드, 실행한
폴더를 기록합니다. 기록은 프로젝트마다 따로가 아니라 컴퓨터 한 곳에 모이므로, `cap`과 `term`은
어느 폴더에서나 쓸 수 있습니다.

```bash
cdg cap "npm run build | tail -20"  # 파이프가 있으면 따옴표로 감싸기
cdg term -i 2         # 두 번 전에 실행한 명령 하나만
cdg term path         # 기록 파일 위치 (CODIGEST_HOME으로 바꿀 수 있음)
cdg term clear        # 기록 모두 지우기
```

`cap` 없이 실행한 명령은 셸의 히스토리(PowerShell, bash, zsh, fish)에서 읽어 오며, 이때는 출력이
없습니다.

#### 자동 기록

`cdg term install`은 설치되어 있는 셸에 맞춰 PowerShell 프로필, `~/.bashrc`, `~/.zshrc`에 훅을
추가합니다. `--shell powershell|bash|zsh|all`로 고를 수 있고, Windows의 Git Bash는 bash로 칩니다.
명령은 원래 세션에서 그대로 실행되므로 `cd`, 변수, 함수가 모두 평소처럼 동작하고, 히스토리에도
입력한 그대로 남습니다. 편집기, 페이저, REPL, `ssh`는 기록하지 않습니다.

- 명령 하나만 기록하지 않으려면 맨 앞에 공백을 넣습니다.
- 기록하지 않을 프로그램을 더 지정하려면: `$env:CODIGEST_CAP_SKIP = '^\s*(myrepl|mytui)\b'`
  (bash/zsh에서는 `export CODIGEST_CAP_SKIP='^(myrepl|mytui)( |$)'`, 확장 정규식).
- 지금 세션에서만 끄려면: `$env:CODIGEST_NOCAP = 1` (bash/zsh: `export CODIGEST_NOCAP=1`).
- 훅을 없애려면: `cdg term uninstall`.

알아 둘 점:

- 기록되는 프로그램은 터미널이 아니라 파이프로 출력하게 되므로, 색이나 진행 막대가 사라지는
  프로그램이 있습니다.
- bash(4 이상)와 zsh에서는 여러 줄에 걸친 명령(여러 줄로 입력한 반복문, heredoc), 백그라운드
  작업(`cmd &`), `!` 히스토리 확장을 쓴 줄은 기록하지 않고 그냥 실행합니다. bash에서는 서브셸
  `( ... )`로 시작하는 줄도 마찬가지입니다.
- bash 훅은 DEBUG 트랩을 씁니다. 첫 프롬프트 전에 설정된 트랩(starship, bash-preexec)은 그대로
  두지만, 그 뒤에 새로 설정한 DEBUG 트랩은 훅을 덮어씁니다.

### MCP 도구

| 도구 | 돌려주는 내용 |
| --- | --- |
| `pack` | 마지막 실패와 관련 파일, 변경 사항, import를 예산에 맞춰 (`targets`, `budget`, `message`) |
| `terminal_history` | 최근 명령과 출력, 종료 코드 (`count`, `failed`, `here`: 이 프로젝트만, 기본값 켜짐) |
| `scan` | 프로젝트 전체 묶음 (`targets`, `resolve`, `message`) |
| `diff` / `semdiff` | 기준점 이후의 diff / 구조 변경 |
| `digest` | 선언과 docstring만 |
| `tree` | 파일 트리 |

각 도구는 CLI가 출력하는 것과 똑같은 내용을 돌려줍니다. 기준점을 옮기는 도구는 없고, 서버를 실행한
프로젝트 밖의 경로는 거부합니다.

### 설정

`cdg init`을 실행하면 `.codigest/config.toml`이 생깁니다. `[filter]` 항목에서 모든 명령이 다룰
파일을 정합니다.

```toml
[filter]
# 이 확장자만 다룹니다 (지우면 무시 목록에 없는 모든 파일)
extensions = [".py", ".ts", ".rs", ".md", ".json"]

# 추가로 뺄 파일, .gitignore 문법 (.gitignore는 항상 따릅니다)
exclude_patterns = [
    "*.lock",
    "tests/data/",
    "legacy_code/"
]
```

`.git/`, `.codigest/`, 가상환경, `node_modules/`, `__pycache__/`는 설정과 상관없이 항상 뺍니다.

### 개인정보와 안전

- **기록 속 비밀정보.** API 키, 토큰, `password=...`, 개인 키, URL에 들어 있는 인증 정보처럼 흔한
  형태는 디스크에 저장하기 전에 가립니다. `env`, `printenv`, `gh auth token`, `aws configure`처럼
  인증 정보를 출력하는 명령은 출력을 아예 저장하지 않습니다. 다만 모든 형태의 비밀정보를 잡아낼
  수는 없습니다.
- **비밀정보 파일.** `pack`은 `.env*`, `*.pem`, `*.key`, `*.p12`, `*.pfx`, SSH 개인 키를 넣지 않고,
  뺐다는 사실만 목록에 남깁니다.
- **내 프로젝트.** 어떤 명령도 소스 파일을 수정하지 않습니다. 저장하는 결과와 기준점은 모두
  `.codigest/` 폴더 안에만 두며, `init`이 이 폴더를 `.gitignore`에 추가합니다. 기준점을 잡지 않았다면
  `pack`은 파일을 하나도 만들지 않습니다.
- **기록 파일 위치.** `cdg term path`로 확인할 수 있습니다. macOS와 Linux에서는 본인만 읽을 수 있게
  저장하고, `cdg term clear`로 지울 수 있습니다.
- **외부로 보내지 않습니다.** Codigest는 네트워크를 쓰지 않습니다. 결과는 내가 붙여 넣은 곳이나
  내가 연결한 MCP 클라이언트로만 갑니다.

### 동작 방식

- **기준점.** `.codigest/anchor/.shadow_git`에 있는 별도의 Git 저장소입니다. 편집기가 프로젝트
  저장소로 착각하지 않도록 이름을 `.git`으로 짓지 않았습니다. `mark`와 `init`이 현재 상태를 여기에
  커밋하고, `diff`, `semdiff`, `status`, `pack`이 지금 파일과 비교합니다.
- **터미널 기록.** 한 줄에 하나씩 JSON으로 적는 파일 하나에 최근 명령 200개를 보관합니다. 출력은
  명령 하나당 64 KB까지 저장하고, 넘치면 앞부분과 뒷부분만 남깁니다.
- **출력 형식.** 파일 내용은 `<file>` 블록 안에 XML 이스케이프해서 넣고, 블록을 들여쓰지 않아
  프롬프트에 불필요한 공백이 섞이지 않게 합니다.
- **MCP.** stdio로 JSON-RPC를 주고받는 작은 서버입니다. 각 도구는 일반 CLI를 `-s --no-copy`로
  실행하므로, 에이전트는 내가 붙여 넣을 내용과 똑같은 것을 받습니다.

### 개발

```bash
git clone https://github.com/NaNIsaac/codigest
cd codigest
uv run cdg --help              # 내려받은 소스에서 바로 실행
uv run --with pytest pytest -q # 테스트
```

데모 GIF는 실제로 기록한 출력을 [`docs/demo/make_gif.py`](docs/demo/make_gif.py)가 그려서 만듭니다.
다시 녹화하는 방법은 그 파일 맨 위의 설명에 있습니다.

## 라이선스

MIT. [LICENSE](LICENSE)를 참고하세요.
