# Codigest

[![PyPI](https://img.shields.io/pypi/v/codigest)](https://pypi.org/project/codigest/) [![Tests](https://github.com/NaNIsaac/codigest/actions/workflows/ci.yml/badge.svg)](https://github.com/NaNIsaac/codigest/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**AI에게 보여 줄 코드를 내가 고르게 해 주는 도구. 프로젝트 전체든, 파일 몇 개든, 바뀐 부분이든, 실패한 명령이든.**

[English](README.md)

<p align="center">
  <a href="https://mcprush.com/to/codigest-mcp?ref=badge">
    <img src="https://mcprush.com/badge/codigest-mcp/card.svg"
         alt="Codigest MCP — MCPRush 보안 등급"
         width="320" loading="lazy">
  </a>
  <br>
  <sub>MCPRush의 자동 보안 평가(A~F)로, 특정 릴리스에 대한 검사 결과입니다. 독립 보안 감사나 안전성 인증은 아닙니다.
  <a href="https://mcprush.com/trust">등급 기준 보기</a>.</sub>
</p>

Codigest는 ChatGPT나 Claude, 코딩 에이전트에게 코드에 대해 물어볼 때 쓰는 명령줄 도구입니다.
프로젝트에서 내가 고른 부분을 하나의 텍스트로 묶어 주므로, 어느 채팅창에든 그대로 붙여 넣을 수
있습니다.

모델은 내가 보여 준 만큼만 압니다. 파일을 손으로 복사하면 번거롭고, 정작 중요한 파일 하나를
빼먹기 쉽고, 반대로 질문과 상관없는 코드까지 잔뜩 붙여 넣기도 쉽습니다. Codigest에서는 무엇을
보여 줄지 내가 정하고, 모으는 일은 도구가 합니다. 프로젝트 전체, 파일 하나와 그 파일이 import하는
파일들, 표시해 둔 시점 이후에 바뀐 부분, 실패한 테스트와 오류가 가리키는 코드 같은 것들입니다.

## 하는 일

- **고른 코드를 묶습니다.** 프로젝트 전체, 지정한 파일과 폴더, 또는 그 파일이 import하는 프로젝트
  파일(Python, JS/TS)을 파일 트리와 함께 하나로 묶습니다.
- **바뀐 부분을 보여 줍니다.** 내가 잡아 둔 기준점 이후의 diff를 텍스트로, 또는 함수 단위로
  보여 줍니다. Git 기록은 건드리지 않습니다.
- **터미널 출력을 가져옵니다.** 실행한 명령을 기록해 두고, 실패한 명령을 오류가 가리키는 파일과
  함께 넘겨줍니다.
- **무엇을 담았는지 알려 줍니다.** 묶을 때마다 크기를 보여 주고, `pack`은 파일마다 넣은 이유와
  뺀 파일 이름까지 알려 줍니다.
- **비밀정보를 막습니다.** `.env`와 키 파일은 절대 넣지 않고, 코드와 출력 속 토큰은 가립니다.
- **에이전트와 연결됩니다.** Claude Code, Cursor 같은 MCP 클라이언트가 같은 결과를 직접 받아 갑니다.

Codigest는 AI 모델을 호출하지 않습니다. 모델에게 보낼 자료를 준비할 뿐입니다. 모든 작업은 내
컴퓨터 안에서 이루어집니다.

<p align="center">
  <img src="docs/demo/demo.gif" alt="cdg cap으로 실패하는 pytest를 실행한 뒤, cdg pack이 실패 출력과 오류가 가리키는 테스트 파일, 바뀐 파일을 묶는 화면" width="760">
  <br>
  <sub>아래 사용법 중 하나: 테스트가 실패한 뒤, 명령 하나로 모델에게 보낼 자료를 모으는 모습</sub>
</p>

## 설치

빌드된 바이너리를 설치합니다. Python이 필요 없습니다. Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.ps1 | iex
```

Linux, macOS, Termux:

```bash
curl -fsSL https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.sh | sh
```

`cdg`는 Windows에서는 `%LOCALAPPDATA%\Programs\codigest`(사용자 `PATH`에 추가), Linux와 macOS에서는
`~/.local/bin`, Termux에서는 `$PREFIX/bin`에 설치됩니다. 같은 줄을 다시 실행하면 업데이트됩니다.
`CDG_VERSION=0.12.1`로 버전을 고정하고, `CDG_INSTALL_DIR=...`로 폴더를 정합니다(이때는 `PATH`를
건드리지 않습니다). 지원 환경은 Windows x64, Linux x64와 ARM64, macOS Intel과 Apple Silicon,
Android ARM64(Termux)입니다(Termux 안에서도 CI로 테스트합니다). 직접 받는 압축 파일은
[릴리스 페이지](https://github.com/NaNIsaac/codigest/releases)에 있습니다. 프로젝트마다 따로 설정할
것은 없습니다.

### Python 패키지

같은 명령을 PyPI에 `codigest`라는 이름으로도 올려 두었습니다. [uv](https://docs.astral.sh/uv/)로
설치하면 필요한 Python(3.15)까지 알아서 받아 두므로, 내 프로젝트의 Python 환경과 섞이지 않습니다.

```bash
winget install --id astral-sh.uv                  # Windows (uv가 이미 있으면 건너뛰기)
curl -LsSf https://astral.sh/uv/install.sh | sh   # macOS/Linux

uv tool install codigest
uv tool update-shell                              # 끝나면 터미널을 다시 열기
```

명령 이름은 `codigest`이고, 줄여서 `cdg`로도 쓸 수 있습니다. 두 방식 모두 `cdg`라는 명령을
설치하므로 `PATH`에는 하나만 두세요. 설치할 때 `lazy import` 부분에서 문법 오류가 나면
`uv self update`를 한 뒤 다시 설치하세요.

다음으로 `cdg term install`을 실행하면 셸에 훅이 설치되어 모든 명령의 출력이 `cdg pack`과
`cdg term`을 위해 보관됩니다. 직접 실행하기 전에는 꺼져 있습니다. [터미널 기록](#터미널-기록)을
참고하세요.

PowerShell의 `help codigest`는 `codigest --help`와 달리 PowerShell의 `Get-Help`를
호출합니다. 터미널 자동 기록 기능을 켜지 않고도 다음처럼 별도로 연결할 수 있습니다.

```powershell
cdg shell-help install       # PowerShell 프로필에 도움말 등록
# PowerShell 새 창 열기
help codigest               # 또는 Get-Help codigest
cdg shell-help uninstall     # 등록 취소
```

다른 PowerShell 명령의 `help` 동작은 바꾸지 않고, Codigest CLI를 연결하는 함수만 추가합니다.

터미널에서 실행하면 아래 명령은 모두 결과를 클립보드에 복사합니다. `-s`를 붙이면 화면에도
출력하고, `-o 파일`을 붙이면 파일로도 저장하며, `--no-copy`를 붙이면 클립보드를 건드리지
않습니다. `-m "..."`로 질문이나 요청을 함께 넣을 수 있습니다. 출력이 파이프나 파일로 갈 때는
(`cdg scan > ctx.xml`, `cdg digest | less`, 스크립트, 코딩 에이전트의 셸) 결과를 그대로 출력하고
클립보드는 건드리지 않습니다. 그래도 복사하려면 `--copy`를 붙입니다.

## 1. 프로젝트를 AI 채팅에 보여 주기

프로젝트 폴더에서 `scan`을 실행하고 붙여 넣으면 됩니다.

```bash
cdg scan
```

```console
$ cdg scan
Copied snapshot to clipboard
Snapshot 3 files, ~121 tokens
```

클립보드에는 `.gitignore`를 따른 파일 트리와 모든 파일이 들어갑니다.

```xml
<project_snapshot project="project">
<project_structure>
├── .gitignore
├── prices.py
└── test_prices.py
</project_structure>
<source_code>
<file path=".gitignore">
.venv/
__pycache__/
.pytest_cache/
</file>

<file path="prices.py">
def apply_discount(price, percent):
    return price * (1 - percent)
</file>

<file path="test_prices.py">
from prices import apply_discount


def test_ten_percent():
    assert apply_discount(200, 10) == 180
</file>
</source_code>
</project_snapshot>
```

프로젝트가 크면(3만 토큰 또는 파일 100개 이상) 복사하기 전에 물어봅니다. 그럴 때는 다음
방법으로 일부만 보여 주세요.

## 2. 일부만, 또는 바뀐 부분만 보여 주기

```bash
cdg scan src/app.py -r       # 파일 하나와, 그 파일이 import하는 프로젝트 파일 전부
cdg scan src/billing/ docs/  # 폴더나 파일 여러 개
cdg scan src/app.py -l       # 줄 번호를 붙여서 ("42번째 줄"처럼 답할 수 있게)
cdg scan src/app.py:40-80    # 40~80번째 줄만, 번호를 붙여서 (한 줄은 src/app.py:42)
cdg digest                   # 선언과 docstring만, 훨씬 짧음
cdg digest src/billing/      # 폴더 하나만
cdg tree -L 2                # 파일 트리, 두 단계까지만
```

여기부터의 예시는 같은 프로젝트가 조금 자란 뒤입니다. `prices.py`에 `Cart` 클래스가 생겼고,
`legacy/`에 예전 MATLAB 버전이 있습니다.

`digest`는 대화를 시작할 때 보여 주기 좋은 개요입니다. Python 파일의 함수, 클래스, 최상위 변수와
MATLAB 파일의 함수, `%%` 섹션을 나오는 순서대로, 시작하는 줄 번호와 docstring(MATLAB은 도움말
주석)의 첫 줄을 붙여 보여 줍니다. 클래스 속성과 dataclass 필드는 값과 함께 나오며, 길면 줄여서
보여 줍니다.

```console
$ cdg digest -s
<code_digest project="project" contents="signatures and docstrings, no bodies">
<project_structure>
├── .gitignore
├── legacy
│   └── discount.m
├── prices.py
└── test_prices.py
</project_structure>
<file path="legacy/discount.m">
1: function p = discount(price, percent)
       Price after taking off percent percent.
3: %% Apply
</file>
<file path="prices.py">
 1: TAX_RATE = 0.1
 4: def apply_discount(price: float, percent: float) -> float
        Price after taking off `percent` percent.
 9: class Cart
        Items and the discount that applies to all of them.
12:   def __init__(self, percent: float=0)
16:   def total(self) -> float
          Sum after discount and tax.
</file>
<file path="test_prices.py">
4: def test_ten_percent()
</file>
</code_digest>
Digest 3 files, ~192 tokens
```

이걸 본 모델은 필요한 것만 콕 집어 달라고 할 수 있고("`Cart.total`을 보여 줘"),
`cdg scan prices.py:16-19`는 그 메서드만 같은 줄 번호로 보여 줍니다.

`tree -L`을 쓰면 크거나 외부 라이브러리가 들어 있는 폴더가 출력을 덮지 않습니다.

```console
$ cdg tree -L 1
├── .gitignore
├── legacy/ (1 file)
├── prices.py
└── test_prices.py
```

작업하면서 바뀐 부분을 보여 주려면 먼저 기준점을 잡아 둡니다.

```bash
cdg mark                     # 예: 리팩터링을 시작하기 전에
# ... 작업 ...
cdg diff                     # 기준점 이후의 diff
cdg semdiff                  # Python: 어떤 함수와 클래스가 바뀌었는지
```

기준점은 `.codigest/` 폴더에 따로 저장하는 복사본이라 Git 기록에는 영향이 없습니다.

## 3. 실패한 명령 넘겨주기

명령을 `cdg cap`으로 실행합니다. 화면에는 평소와 똑같이 출력되고, Codigest는 그 내용을 따로
저장해 둡니다. 그다음 `pack`을 실행합니다.

```bash
cdg cap -- pytest -q        # npm test, cargo test, go test ./..., make 무엇이든 됩니다
cdg pack
```

```console
$ cdg pack
Packed ~370 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py  line 5 in failing output
  + prices.py       changed
Copied pack to clipboard
```

위에서부터 실패한 명령, Git에서 아직 커밋하지 않은 변경, 오류가 5번째 줄을 가리킨
`test_prices.py`, 내가 고친 `prices.py`를 담았다는 뜻입니다.

<details>
<summary>클립보드에 실제로 들어간 내용</summary>

```xml
<context_pack project="project">
<terminal_history>
<command index="1" command="pytest -q" exit_code="1" cwd="~/project" duration="1.5s" at="2026-10-17T01:58:14+09:00">
<output>
F                                                                      [100%]
================================= FAILURES ==================================
_____________________________ test_ten_percent ______________________________

    def test_ten_percent():
>       assert apply_discount(200, 10) == 180
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

### `pack`이 파일을 고르는 기준

그냥 `cdg pack`을 실행하면 아래 순서대로 토큰 예산(기본 30,000)을 채웁니다.

1. 이 프로젝트에서 최근 한 시간 안에 실패한 마지막 명령의 출력에 나온 파일.
   `File "x.py", line 7`(Python), `x.py:7`(pytest, ruff, gcc, rustc, eslint), `x.ts(7,3)`(tsc)
   같은 형식을 알아봅니다. 같은 명령이 나중에 성공하면 그 실패는 더 이상 쓰지 않습니다.
   내가 고치지 않았는데 출력에만 나온 파일은, 오류가 가리킨 곳마다 앞뒤 열 줄씩만 잘라서 넣고
   `lines="40-60"`처럼 표시합니다. 짧은 파일은 전체를 넣습니다.
2. 실패한 Python 테스트가 호출하는 프로젝트 함수와 클래스. 전체를 넣고 이유는
   `called from failing test ...`입니다. assert에서 끝나는 traceback은 테스트만 가리키므로, 버그가
   이미 커밋되어 diff에 아무것도 없을 때도 테스트 대상 코드를 함께 넣기 위한 것입니다.
3. 내가 바꾼 파일: Git에서 아직 커밋하지 않은 파일과 새로 만든 파일, 또는 `cdg mark` 이후의 변경.
   긴 파일은 diff hunk가 닿는 부분만 넣습니다(diff 자체는 이미 pack에 들어 있습니다).
4. 그 파일들을 import하는 테스트(`test_*.py`, `*_test.py`, `*.test.ts`, `*.spec.js` 등).
5. 그 파일들이 import하는 프로젝트 파일. Python import와 JS/TS 상대 경로 import를 따라갑니다
   (`./x`, `x.ts`를 가리키는 `./x.js`, `dir/index.ts`를 가리키는 `./dir`).

파일을 직접 지정하면 그 파일만 넣습니다. 자동으로 넣던 것들은 `--with`로 다시 켤 수 있습니다.

```bash
cdg pack src/prices.py                         # 이 파일만
cdg pack src/prices.py --with tests            # 이 파일과 그 테스트
cdg pack --with changes                        # 실패는 빼고 내 변경만
cdg pack --with failure,changes,tests,imports  # 그냥 `cdg pack`과 같음
```

실패한 명령과 diff는 해당 항목이 켜져 있으면 항상 넣습니다. 명령 출력이 길면 앞 80줄과 뒤 40줄(처음 몇
개의 실패와 요약)만 남기며, 전체는 `cdg term -f`로 볼 수 있습니다. 토큰 수는 네 글자를 한 토큰으로
계산한 어림값이라 실제와 조금 다를 수 있습니다.

### 더 편하게 쓰기

`cdg term install`로 셸에 훅을 설치하면 `cdg cap`을 붙이지 않아도 모든 명령이 기록됩니다
(`cdg term uninstall`로 제거). 기록된 내용은 `cdg term`으로 봅니다.
[터미널 기록](#터미널-기록)을 참고하세요.

## 4. 에이전트가 직접 가져가게 하기

코딩 에이전트를 MCP로 연결하면, 붙여 넣어 달라고 하는 대신 에이전트가 직접 가져갑니다.

```bash
claude mcp add --scope user codigest -- cdg mcp      # Claude Code
```

Cursor, VS Code, Claude Desktop 같은 다른 클라이언트에서는 명령 `cdg`, 인자 `["mcp"]`로 stdio
서버를 추가하면 됩니다. 제공하는 도구는 [MCP 도구](#mcp-도구)에 있습니다.

셸 명령을 실행할 수 있는 에이전트라면 아무 설정도 필요 없습니다. 에이전트의 셸은 터미널이 아니라서
`cdg`가 결과를 그대로 출력해 주고, 내 클립보드는 건드리지 않습니다. 에이전트에게 맞는 사용 순서는
이렇습니다.

```bash
cdg tree -L 2                      # 어디에 무엇이 있는지
cdg digest src/                    # 무엇이 어디에 정의되어 있는지, 줄 번호와 함께
cdg scan src/prices.py:16-19      # 필요한 부분만, 같은 줄 번호로
cdg mark                           # 고치기 전에
cdg semdiff                        # 고친 뒤: 어떤 함수가 바뀌었는지
cdg cap -- pytest -q && cdg pack   # 테스트 실행, 실패하면 pack에 고칠 거리가 모임
```

모든 블록의 코드는 파일에 적힌 그대로입니다(`->`, `<`, `&`를 이스케이프하지 않음). 그래서 블록에서
복사한 코드를 그대로 붙여 넣을 수 있습니다. 대답해 줄 사람이 없을 때 큰 `scan`은 크기를 묻고
기다리는 대신 안내와 함께 멈춥니다. 전부 받으려면 `-y`를 붙입니다.

## 참고 자료

- [전체 명령](#전체-명령)
- [터미널 기록](#터미널-기록)
- [MCP 도구](#mcp-도구)
- [설정](#설정)
- [개인정보와 안전](#개인정보와-안전)
- [0.9에서 올라올 때](#09에서-올라올-때)
- [동작 방식](#동작-방식)
- [개발](#개발)

### 전체 명령

`cdg --help`도 같은 묶음으로 보여 줍니다.

| 명령 | 하는 일 | 프로젝트에 만드는 파일 |
| --- | --- | --- |
| `scan` | 프로젝트 전체나 지정한 경로를 하나로 묶기 | 없음 |
| `digest` | 줄 번호가 붙은 선언과 docstring, 본문 제외 (Python, MATLAB) | 없음 |
| `tree` | 프로젝트 파일 트리 (`-L N`으로 깊이 제한) | 없음 |
| `mark` | 기준점 잡기 | `.codigest/` |
| `diff` | 기준점 이후의 diff | 없음 |
| `semdiff` | 기준점 이후 함수·클래스 단위 변경 (Python) | 없음 |
| `cap` | 명령을 실행하고 출력 기록 | 없음 |
| `term` | 최근 기록한 명령과 출력 보기 | 없음 |
| `pack` | 마지막 실패와 관련 파일, 변경 사항 묶기 | 없음 |
| `mcp` | 에이전트용 MCP 서버 실행 | 없음 |
| `status` | 지금 `pack`이 쓸 내용 확인: 마지막 실패, 변경 사항, 기준점 | 없음 |
| `init` | [파일 필터](#설정)용 `.codigest/config.toml` 만들기 | `.codigest/`, `.gitignore` |
| `clean` | `.codigest/` 지우기 (먼저 물어보며, `-y`면 묻지 않음) | `.codigest/` 삭제 |

공통 옵션: `-s` / `--stdout`은 결과를 화면에도 출력하고(상태 메시지는 stderr로, 파이프로 보낼
때는 기본으로 켜짐), `--copy` / `--no-copy`는 클립보드 기본값(터미널에서는 복사, 파이프로 보낼
때는 복사 안 함)을 바꾸며, `-o 파일`은 파일로도 저장하고, `-m` / `--message`는 요청 문구를 덧붙입니다.
`scan -l`은 줄 번호를 붙이고, `scan -a`는 설정 필터를 무시하며, `scan -y`는 확인 없이
진행합니다(CI용). `scan`, `diff`, `semdiff`, `digest`에 `-r` / `--resolve`를 붙이면 대상 파일이
import하는 프로젝트 파일도 함께 넣습니다. `scan`과 `term`은 `--format markdown|json`을 받습니다(기본은 XML).
Markdown은 이를 렌더링하는 채팅용, JSON은 스크립트용입니다. 어떤 명령에든 `--preset 이름`을 붙이면
`.codigest/config.toml`이나 사용자 설정의 `[presets.<명령>.이름]`에 저장해 둔 옵션이 채워지고, 그 뒤에 직접
쓴 옵션이 우선합니다. `cdg <명령> --help`로 모든 옵션을 볼 수 있습니다.

### 터미널 기록

`cdg cap`은 명령의 출력을 그대로 보여 주면서 명령어, 출력(stdout과 stderr), 종료 코드, 실행한
폴더를 기록합니다. `cdg term --context`를 쓰면 **명령 실행 당시의 PC 이름과 셸**도
복사 내용에 포함합니다. PC 이름은 개인정보일 수 있어 기본적으로는 표시하지 않으며,
예전에 저장한 기록에는 PC 이름이 없을 수 있습니다. 기록은 프로젝트마다 따로가 아니라
컴퓨터 한 곳에 모이므로, `cap`과 `term`은 어느 폴더에서나 쓸 수 있습니다.

```bash
cdg cap "npm run build | tail -20"  # 파이프가 있으면 따옴표로 감싸기
cdg term              # 마지막 명령과 출력
cdg term -n 3         # 최근 3개
cdg term -f           # 마지막으로 실패한 명령
cdg term --context   # 복사 내용에 실행 PC 이름과 셸 정보 추가
cdg term --here -n 3  # 이 프로젝트에서 실행한 최근 3개
cdg term -i 2         # 두 번 전에 실행한 명령 하나만
cdg term -n 3 --format json  # 최근 3개를 JSON으로 (markdown도 가능)
cdg term path         # 기록 파일 위치 (CODIGEST_HOME으로 바꿀 수 있음)
cdg term clear        # 기록 모두 지우기
```

`cap` 없이 실행한 명령은 셸의 히스토리(PowerShell, bash, zsh, fish)에서 읽어 오며, 이때는 출력이
없습니다.

#### 자동 기록

`cdg term install`은 설치되어 있는 셸에 맞춰 PowerShell 프로필, `~/.bashrc`, `~/.zshrc`에 훅을
추가합니다. `--shell powershell|bash|zsh|all`로 고를 수 있고, Windows의 Git Bash는 bash로 칩니다.
명령은 원래 세션에서 그대로 실행되므로 `cd`, 변수, 함수가 모두 평소처럼 동작하고, 히스토리에도
입력한 그대로 남습니다. 실제 터미널이 필요한 프로그램은 기록하지 않습니다. 편집기, 페이저, `ssh`, 전체 화면 도구(`tmux`, `zellij`, `herdr`, `k9s`, `yazi` 등), `-it`가 붙은 `docker`/`podman`/`kubectl`, `npm`/`pnpm`/`yarn`/`bun`의 `init`·`create`, `npx create-*`, `pip uninstall`, `gh auth login`, `az login`, `gcloud auth login`·`gcloud init`, `aws configure`, `ssh-keygen`, `sudo`, 데이터베이스 셸(`psql`, `mysql`, `sqlite3` 등)이 해당합니다. 줄바꿈으로 끝나지 않는 질문은 기록용 파이프 안에서 다음 줄바꿈이 올 때까지 화면에 나타나지 않아, 명령이 멈춘 것처럼 보이기 때문입니다.

- 명령 하나만 기록하지 않으려면 맨 앞에 공백을 넣습니다.
- 기록하지 않을 프로그램을 더 지정하려면: `$env:CODIGEST_CAP_SKIP = '^\s*(myrepl|mytui)\b'`
  (bash/zsh에서는 `export CODIGEST_CAP_SKIP='^(myrepl|mytui)( |$)'`, 확장 정규식).
- 지금 세션에서만 끄려면: `$env:CODIGEST_NOCAP = 1` (bash/zsh: `export CODIGEST_NOCAP=1`).
- 훅을 없애려면: `cdg term uninstall`.

프로필은 패키지 안의 파일이 아니라, `cdg term install`이 데이터 폴더(기록 옆의 `shell/`)에 둔 훅 사본을 불러옵니다. codigest를 업데이트한 뒤에는 `cdg term install`을 다시 실행해 사본을 갱신하세요.

알아 둘 점:

- 기록되는 프로그램은 터미널이 아니라 파이프로 출력하게 되므로, 색이나 진행 막대가 사라지는
  프로그램이 있습니다.
- bash(4 이상)와 zsh에서는 여러 줄에 걸친 명령(여러 줄로 입력한 반복문, heredoc), 백그라운드
  작업(`cmd &`), `!` 히스토리 확장을 쓴 줄은 기록하지 않고 그냥 실행합니다. bash에서는 서브셸
  `( ... )`로 시작하는 줄도 마찬가지입니다.
- 긴 출력에는 상한이 있습니다. PowerShell 훅은 앞뒤 2,000줄씩만 보관하고, bash와 zsh는 기록 파일이 4 MB를 넘으면 앞뒤에서 1 MB씩 읽습니다. 그다음 저장소는 64 KB만 남깁니다.
- PowerShell에서는 종료 코드가 0이 아니거나 오류가 화면에 나오면 실패로 칩니다. `-ErrorAction`으로 감춘 오류는 실패가 아닙니다.
- PowerShell 훅은 원격 세션, 중첩 프롬프트, 디버거에서는 줄을 감싸지 않습니다.
- bash 훅은 DEBUG 트랩을 씁니다. 첫 프롬프트 전에 설정된 트랩(starship, bash-preexec)은 그대로
  두지만, 그 뒤에 새로 설정한 DEBUG 트랩은 훅을 덮어씁니다.

### MCP 도구

| 도구 | 돌려주는 내용 |
| --- | --- |
| `pack` | 마지막 실패와 관련 파일, 변경 사항, import를 예산에 맞춰 (`targets`, `with`, `budget`, `message`) |
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

- **인증 정보 파일.** `.env*`(`.env.example` 같은 템플릿은 제외), `*.pem`, `*.key`, `*.p12`,
  `*.pfx`, 키스토어, `.netrc`, `.pgpass`, SSH 개인 키는 어떤 명령도 넣지 않습니다. `scan`은 뺀
  파일을 알려 주고, `pack`은 제외 목록에 남기며, diff에서는 해당 부분을 안내 문구로 바꿉니다.
- **코드와 diff 속 토큰.** 개인 키, JWT, 주요 서비스 토큰(OpenAI, GitHub, AWS, Google, Slack,
  GitLab, npm, Stripe `sk_`/`rk_`, Hugging Face `hf_`), URL에 들어 있는 비밀번호는 Codigest가 내보내는 모든 결과에서 가립니다.
  `token = get_token()` 같은 평범한 코드는 그대로 둡니다.
- **터미널 기록 속 비밀정보.** 여기에 더해 터미널 출력은 디스크에 저장하기 전에 `Authorization`
  헤더와 `key=value`, `key: value` 형태의 값도 가립니다. 키에 따옴표가 있거나(`"password": "..."`)
  `_`, `-` 뒤에 비밀 이름이 붙은 키(`AWS_SECRET_ACCESS_KEY`, `DB_PASS`, `pass`, `pwd` 포함)도 잡습니다.
  인증 정보를 출력하는 명령은 출력을 아예 저장하지 않습니다: `env`, `printenv`, `gh auth token`,
  `aws configure`, `aws sts get-session-token`/`assume-role`, `az account get-access-token`,
  `gcloud auth print-access-token`, `kubectl get secret`, `kubectl config view --raw`, `op read`,
  `bw get`, `pass show`, `vault read`, `.env`·`.npmrc`·`.pypirc`·`.netrc`·키 파일·`credentials.json`을
  `cat`하는 명령 등입니다(전체 목록은 `src/codigest/core/redact.py`의 `DENY_COMMANDS`에 있습니다).
- 이 규칙들은 흔한 형태를 잡아내는 것이고, 모든 비밀정보를 잡을 수는 없습니다.
- **내 프로젝트.** 어떤 명령도 소스 파일을 수정하지 않습니다. `.codigest/` 폴더는 `mark`와
  `init`만 만들며, 폴더 안의 `.gitignore`(`*`) 덕분에 `git status`에 나타나지 않습니다.
  `init`은 프로젝트의 `.gitignore`에도 이 폴더를 추가합니다.
- **기록 파일 위치.** `cdg term path`로 확인할 수 있습니다. macOS와 Linux에서는 본인만 읽을 수 있게
  저장하고, `cdg term clear`로 지울 수 있습니다.
- **외부로 보내지 않습니다.** Codigest는 네트워크를 쓰지 않습니다. 결과는 내가 붙여 넣은 곳이나
  내가 연결한 MCP 클라이언트로만 갑니다.

### 0.9에서 올라올 때

- 파이프나 리다이렉트로 보낸 출력은 `-s` 없이도 출력되고 클립보드에 복사되지 않습니다. 스크립트가
  클립보드에 기대고 있었다면 `--copy`를 붙이세요.
- `<file>` 블록에 코드를 XML 이스케이프하지 않고 그대로 넣습니다. `.codigest/prompts.toml`의
  사용자 템플릿도 블록을 이스케이프 없이 받으며, `{instruction}`만 이스케이프합니다.
- `digest` 출력에 줄 번호와 docstring이 붙고, 소스 순서대로 나오며, 경로를 여러 개 받습니다.

### 0.8에서 올라올 때

- `scan`, `diff`, `semdiff`, `digest`가 더 이상 `.codigest/*.txt`에 저장하지 않습니다. 파일이
  필요하면 `-o 파일`을 쓰세요. `diff --no-save` 같은 옵션은 없어졌습니다.
- `cdg init`이 더 이상 기준점을 잡지 않습니다. `cdg mark`를 실행하세요.
- `cdg pack 파일`은 그 파일만 넣습니다. 예전처럼 하려면
  `--with failure,changes,tests,imports`를 붙이세요.
- 결과 맨 앞에 붙던 모델용 안내 문구("You are an expert..." 등)가 없어졌습니다. 필요하면 `-m`으로
  직접 넣거나 `.codigest/prompts.toml`에 템플릿을 두세요.

### 동작 방식

- **기준점.** `.codigest/anchor/.shadow_git`에 있는 별도의 Git 저장소입니다. `mark`가 현재 상태를 여기에
  커밋하고, `diff`, `semdiff`, `status`, `pack`이 지금 파일과 비교합니다.
- **터미널 기록.** 한 줄에 하나씩 JSON으로 적는 파일 하나에 최근 명령 200개를 보관합니다. 출력은
  명령 하나당 64 KB까지 저장하고, 넘치면 앞부분과 뒷부분만 남깁니다.
- **출력 형식.** 파일 내용은 `<file>` 블록 안에 그대로 넣습니다. 파일 안에 Codigest 블록의 닫는
  태그(`</file>` 같은)가 그대로 있을 때만 `&lt;/file>`로 바꿔, 블록이 중간에 끝나지 않게 합니다.
- **MCP.** stdio로 JSON-RPC를 주고받는 작은 서버입니다. 각 도구는 일반 CLI를 `-s --no-copy`로
  실행하므로, 에이전트는 내가 붙여 넣을 내용과 똑같은 것을 받습니다.

### 개발

```bash
git clone https://github.com/NaNIsaac/codigest
cd codigest
uv run cdg --help              # 내려받은 소스에서 바로 실행
uv run --with pytest pytest -q # 테스트
```

## 라이선스

MIT. [LICENSE](LICENSE)를 참고하세요.
