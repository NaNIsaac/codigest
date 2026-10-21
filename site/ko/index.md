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
      text: 명령어 레퍼런스
      link: /ko/commands
    - theme: alt
      text: GitHub
      link: https://github.com/NaNIsaac/codigest

install:
  label: 설치
  command: uv tool install codigest
  copy: 복사
  copied: 복사됨

demo:
  alt: cdg cap으로 실패하는 pytest를 실행한 뒤, cdg pack이 실패 출력과 오류가 가리키는 테스트 파일, 바뀐 파일을 묶는 화면
  caption: 테스트가 실패한 뒤, 명령 하나로 모델에게 보낼 자료를 모으는 모습

features:
  - title: 고른 코드를 묶습니다
    details: 프로젝트 전체, 지정한 파일과 폴더, 또는 그 파일이 import하는 프로젝트 파일(Python, JS/TS)을 파일 트리와 함께 하나로 묶습니다.
    link: /ko/commands#cdg-scan
  - title: 바뀐 부분을 보여 줍니다
    details: 내가 잡아 둔 기준점 이후의 diff를 텍스트로, 또는 함수 단위로 보여 줍니다. Git 기록은 건드리지 않습니다.
    link: /ko/commands#cdg-mark
  - title: 터미널 출력을 가져옵니다
    details: 실행한 명령을 기록해 두고, 실패한 명령을 오류가 가리키는 파일과 함께 넘겨줍니다.
    link: /ko/terminal-capture
  - title: 무엇을 담았는지 알려 줍니다
    details: 묶을 때마다 크기를 보여 주고, pack은 파일마다 넣은 이유와 뺀 파일 이름까지 알려 줍니다.
    link: /ko/commands#cdg-pack
  - title: 비밀정보를 막습니다
    details: .env와 키 파일은 절대 넣지 않고, 코드와 출력 속 토큰은 가립니다.
    link: /ko/security
  - title: 에이전트와 연결됩니다
    details: Claude Code, Cursor 같은 MCP 클라이언트가 같은 결과를 직접 받아 갑니다.
    link: /ko/mcp
---

Codigest는 AI 모델을 호출하지 않습니다. 모델에게 보낼 자료를 준비할 뿐입니다. 모든 작업은 내 컴퓨터 안에서 이루어집니다.

## 직접 보기

아래는 작은 프로젝트에서 실제로 실행한 결과입니다. `prices.py`와, 실패하는 테스트 `test_prices.py`로 이루어진 프로젝트입니다. 예시 사이에 프로젝트가 조금씩 자랍니다. `prices.py`에 `Cart` 클래스가 생기고, MATLAB 파일이 든 `legacy/` 폴더가 생깁니다.

### 프로젝트 보여 주기

```console
$ cdg scan -s
```

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

터미널에서 실행하면 결과가 클립보드에 복사됩니다. `-s`를 붙이면 화면에도 출력합니다.

### 개요부터 시작하기

`digest`는 함수, 클래스, 최상위 변수를 시작 줄 번호와 docstring 첫 줄과 함께 나열하고, 본문은 뺍니다.

```console
$ cdg digest -s
```

```xml
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
5: %% Apply
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
```

### 실패한 명령 넘기기

```console
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
1 failed in 0.04s

$ cdg pack
Packed ~689 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py     line 5 in failing output
  + prices.py          called from failing test test_prices.py
  + legacy/discount.m  changed
```

pack에는 위에서부터 실패한 명령과 그 출력, Git에서 아직 커밋하지 않은 변경, 오류가 5번째 줄을 가리킨 `test_prices.py`, 그 테스트가 호출하는 `prices.py`, 내가 고친 `legacy/discount.m`이 들어갑니다. 전체 내용은 [출력 형식](/ko/output-formats)에서 볼 수 있습니다.
