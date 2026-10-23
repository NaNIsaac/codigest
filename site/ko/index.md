---
layout: home
title: Codigest
titleTemplate: AI에게 보여 줄 코드를 내가 고르게 해 주는 도구

hero:
  name: Codigest
  text: AI에게 보여 줄 코드를 내가 고르게 해 주는 도구
  tagline: 프로젝트 전체든, 파일 몇 개든, 바뀐 부분이든, 실패한 명령이든. 어느 채팅창에든 붙여 넣을 수 있는 텍스트 하나로 묶어 줍니다.
  actions:
    - theme: brand
      text: 빠른 시작
      link: /ko/getting-started
    - theme: alt
      text: GitHub
      link: https://github.com/NaNIsaac/codigest

install:
  label: Codigest 설치
  windowsLabel: Windows
  unixLabel: Linux / macOS / Termux
  pythonLabel: Python (uv)
  windows: 'irm https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.ps1 | iex'
  unix: 'curl -fsSL https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.sh | sh'
  python: 'uv tool install codigest'
  copy: 복사
  copied: 복사됨
  moreLabel: 설치 방법 자세히 보기
  moreLink: /ko/installation

demo:
  alt: cdg cap으로 실패하는 pytest를 실행한 뒤, cdg pack이 실패 출력과 커밋하지 않은 변경, 관련 파일 두 개를 묶는 화면
  caption: 테스트가 실패한 뒤, 명령 하나로 모델에게 보낼 자료를 모으는 모습

workflowsMore: 자세히 보기
workflows:
  - title: 프로젝트 전체 보여 주기
    details: 파일 트리와 모든 파일을 하나로 묶습니다. .gitignore와 설정에서 뺀 파일은 들어가지 않습니다.
    commands: ['cdg scan']
    link: /ko/commands#cdg-scan
  - title: 파일 몇 개와 그 파일이 import하는 코드
    details: 파일이나 폴더를 지정합니다. -r을 붙이면 그 파일이 import하는 프로젝트 파일(Python, JS/TS)까지 따라갑니다.
    commands: ['cdg scan src/billing/invoice.py -r']
    link: /ko/recipes#share-one-folder-or-one-question-s-worth-of-files
  - title: 바뀐 부분만 보여 주기
    details: 작업 전에 기준점을 잡아 두면, 그 뒤의 텍스트 diff나 바뀐 Python 함수 목록을 보낼 수 있습니다. Git 기록은 건드리지 않습니다.
    commands: ['cdg mark', 'cdg diff']
    link: /ko/recipes#review-a-refactor-with-mark-and-semdiff
  - title: 실패한 테스트 넘기기
    details: 테스트를 cap으로 실행한 뒤 pack을 만들면, 실패 출력과 오류가 가리키는 파일, 내 변경이 함께 묶입니다.
    commands: ['cdg cap -- pytest -q', 'cdg pack']
    link: /ko/recipes#a-test-fails-and-you-do-not-know-why
  - title: 에이전트가 직접 가져가게 하기
    details: Claude Code, Cursor 같은 MCP 클라이언트가 같은 명령을 호출해 같은 결과를 받습니다.
    commands: ['claude mcp add --scope user codigest -- cdg mcp']
    link: /ko/mcp
---

Codigest는 AI 모델을 호출하지 않습니다. 모델에게 보낼 자료를 준비할 뿐입니다. 모든 작업은 내 컴퓨터 안에서 이루어집니다.

## 직접 보기

작은 프로젝트에서 한 번에 실행한 실제 출력입니다. 방금 `prices.py`를 고쳤고, 그 바람에 `test_prices.py`의 테스트가 실패하는 상황입니다.

::: code-group

```txt [cdg scan]
$ cdg scan -s
Copied snapshot to clipboard
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
"""Prices after discount and tax."""

TAX_RATE = 0.1


def apply_discount(price: float, percent: float) -> float:
    """Price after taking off `percent` percent."""
    return price * (1 - percent)


class Cart:
    """Items and the discount that applies to all of them."""

    def __init__(self, percent: float = 0):
        self.items: list[float] = []
        self.percent = percent

    def total(self) -> float:
        """Sum after discount and tax."""
        return sum(apply_discount(p, self.percent) for p in self.items) * (1 + TAX_RATE)
</file>

<file path="test_prices.py">
from prices import apply_discount


def test_ten_percent():
    assert apply_discount(200, 10) == 180
</file>
</source_code>
</project_snapshot>
```

```txt [cdg digest]
$ cdg digest -s
Digest 2 files, ~160 tokens
Copied digest to clipboard
<code_digest project="project" contents="signatures and docstrings, no bodies">
<project_structure>
├── .gitignore
├── prices.py
└── test_prices.py
</project_structure>
<file path="prices.py">
 1: """Prices after discount and tax."""
 3: TAX_RATE = 0.1
 6: def apply_discount(price: float, percent: float) -> float
        Price after taking off `percent` percent.
11: class Cart
        Items and the discount that applies to all of them.
14:   def __init__(self, percent: float=0)
18:   def total(self) -> float
          Sum after discount and tax.
</file>
<file path="test_prices.py">
4: def test_ten_percent()
</file>
</code_digest>
```

```txt [cdg cap]
$ cdg cap -- pytest -q
F                                                                        [100%]
================================== FAILURES ===================================
______________________________ test_ten_percent _______________________________

    def test_ten_percent():
>       assert apply_discount(200, 10) == 180
E       assert -1800 == 180
E        +  where -1800 = apply_discount(200, 10)

test_prices.py:5: AssertionError
=========================== short test summary info ===========================
FAILED test_prices.py::test_ten_percent - assert -1800 == 180
1 failed in 0.06s
```

```txt [cdg pack]
$ cdg pack
Packed ~545 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py  line 5 in failing output
  + prices.py       called from failing test test_prices.py
Copied pack to clipboard
```

:::

- **scan**은 프로젝트를 블록 하나로 묶습니다. 터미널에서는 클립보드로 복사되고, `-s`를 붙이면 화면에도 출력합니다.
- **digest**는 정의마다 줄 번호와 시그니처, docstring 첫 줄만 남기고 본문은 뺍니다. 모델은 이걸 보고 `apply_discount`를 이름으로 요청할 수 있습니다.
- **cap**은 테스트를 평소처럼 실행하면서 출력을 저장해 둡니다.
- **pack**에는 실패 출력, 커밋하지 않은 변경, 오류가 5번째 줄을 가리킨 `test_prices.py`, 그 테스트가 호출하는 `prices.py`가 들어갑니다. pack 전체 내용은 [출력 형식](/ko/output-formats)에서 볼 수 있습니다.

위 출력은 Windows에서 네이티브 `cdg` 0.12.2로 실행한 결과입니다. Python 패키지도 같은 블록을 출력하며, 두 구현이 다른 부분은 [Rust 포팅](/ko/rust-port#differences-from-the-python-build)에 정리되어 있습니다.

## 자주 쓰는 방법

<HomeWorkflows />

## 무엇을 보내고 무엇을 막는지

<div class="home-grid">
<div>

### 내 컴퓨터 안에서만 동작

Codigest는 네트워크 요청을 하지 않습니다. 내가 붙여 넣거나, 연결해 둔 MCP 클라이언트가 요청할 때만 내용이 밖으로 나갑니다.

</div>
<div>

### 절대 넣지 않는 파일

`.env` 파일, 키와 인증서(`*.pem`, `*.key` 등), SSH 개인 키, `.netrc`, `.pgpass`. 뺀 파일은 명령이 이름을 알려 줍니다.

</div>
<div>

### 가리는 값

GitHub, AWS, OpenAI, Slack 키처럼 형태가 알려진 토큰은 `[REDACTED]`로 바꿉니다. 터미널 기록은 저장하기 전에 가립니다.

</div>
<div>

### 완벽하지는 않습니다

규칙이 모르는 형식의 비밀정보는 걸러지지 않습니다. 붙여 넣기 전에 내용을 확인하세요. 모든 규칙과 한계는 [보안](/ko/security)에 있습니다.

</div>
</div>
