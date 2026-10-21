# 출력 형식

모든 명령은 XML 비슷한 텍스트 블록 하나를 출력합니다. 태그는 모델이 프로젝트 트리, 각 파일, diff, 터미널 실행 결과의 경계를 분명히 알아보게 해 줍니다. Codigest는 모델에게 주는 안내 문구를 따로 넣지 않습니다. 모델이 이 블록으로 무엇을 할지는 `-m`으로 넣은 문구, 또는 내 [템플릿](/ko/configuration#prompts-toml)이 정합니다.

| 명령 | 루트 요소 |
| --- | --- |
| `scan` | `<project_snapshot>` |
| `digest` | `<code_digest>` |
| `diff` | `<changes>` |
| `semdiff` | `<semantic_diff>` |
| `pack` | `<context_pack>` |
| `term` | `<terminal_history>` |
| `tree` | 태그 없는 일반 텍스트 |

## 내 요청 문구

`-m "..."`은 루트의 첫 번째 자식으로 `<user_instruction>` 블록을 넣습니다.

```xml
<project_snapshot project="project">
<user_instruction>
Why does the test fail?
</user_instruction>
<project_structure>
...
```

## 스냅숏: scan

파일 트리가 든 `<project_structure>` 다음에, 파일마다 `<file>`이 하나씩 든 `<source_code>`가 나옵니다. `path`는 프로젝트 루트 기준의 상대 경로이고 슬래시를 씁니다.

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

`-r`로 끌어온 프로젝트 밖의 파일은 `[EXTERNAL]/이름`으로 표시됩니다.

## 코드는 그대로

`<file>` 블록 안의 코드는 파일에 있는 그대로입니다. `->`, `<`, `&`를 이스케이프하지 않으므로, 블록에서 복사한 코드를 그대로 붙여 넣을 수 있고, 모델이 제안한 수정에 `-&gt;`가 섞여 돌아오지도 않습니다.

예외는 하나입니다. 내용 안에 Codigest 자신의 블록을 닫는 태그(`file`, `source_code`, `project_snapshot`, `project_structure`, `code_digest`, `changes`, `semantic_diff`, `user_instruction`, `context_pack`, `omitted`, `terminal_history`, `command`, `output`, `note`)가 있으면 `&lt;`로 바꿔, 내용 때문에 블록이 중간에 끝나지 않게 합니다.

```xml
<file path="html.py">
def parse(s: str) -> dict[str, int]:
    return {k: 1 for k in s if k < "z" and k > "a"}

TEMPLATE = "&lt;/file>"
</file>
```

파일에는 `"</file>"`이 있었고, 비교 연산자와 `->`는 그대로입니다.

## 줄 번호와 범위

`scan -l`은 모든 줄에 번호를 붙입니다. 번호는 마지막 줄 번호의 자릿수에 맞춰 오른쪽으로 정렬하고, 콜론과 공백이 뒤따릅니다.

범위로 지정한 대상(`scan prices.py:16-19`)은 보여 주는 줄에 번호를 붙이고 `lines` 속성을 넣어, 읽는 쪽이 파일의 일부라는 것을 알 수 있게 합니다.

```xml
<file path="prices.py" lines="16-19">
16:     def total(self) -> float:
17:         """Sum after discount and tax."""
18:         subtotal = sum(apply_discount(p, self.percent) for p in self.items)
19:         return subtotal * (1 + TAX_RATE)
</file>
```

`pack`도 발췌를 같은 방식으로 표시합니다. 실패 출력에 이름만 나온 긴 파일은 출력이 가리킨 곳마다 앞뒤 열 줄씩 잘라 넣고, 떨어진 구간 사이에는 `...`를 넣습니다. 아래는 출력에 `pipeline.py:50`이 나왔을 때입니다.

```xml
<file path="pipeline.py" reason="line 50 in failing output" lines="40-60">
 40: def step_14(x):
 41:     return x + 14
 42: 
 43: def step_15(x):
 44:     return x + 15
 45: 
 46: def step_16(x):
 47:     return x + 16
 48: 
 49: def step_17(x):
 50:     return x + 17
 51: 
 52: def step_18(x):
 53:     return x + 18
 54: 
 55: def step_19(x):
 56:     return x + 19
 57: 
 58: def step_20(x):
 59:     return x + 20
 60: 
</file>
```

`digest`의 번호는 각 선언이 시작하는 줄입니다.

## 개요: digest

```xml
<code_digest project="project" contents="signatures and docstrings, no bodies">
<project_structure>
...
</project_structure>
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
</code_digest>
```

항목은 소스에 나오는 순서입니다. 메서드는 클래스 아래에 들여 쓰고, docstring의 첫 줄은 해당 항목 아래에 놓입니다.

## 변경: diff와 semdiff

`diff`는 unified diff를 `<changes>`로 감쌉니다. `source` 속성은 무엇과 비교했는지 알려 줍니다.

```xml
<changes project="project" source="since cdg mark (1 second ago)">
diff --git a/prices.py b/prices.py
...
</changes>
```

`semdiff`는 파일마다 표지와, 해당 항목이 시작하는 줄 번호를 나열합니다.

```xml
<semantic_diff project="project" source="since cdg mark (1 second ago)">
<file path="prices.py">
[SIGNATURE] 16: method Cart.total :: (self) -> float -> (self, tax: bool=True) -> float
[LOGIC]     4: function apply_discount
[ADDED]     22: function tax_of
</file>
</semantic_diff>
```

## 터미널 기록: term

기록마다 `<command>` 하나가 나옵니다. `index="1"`이 가장 최근입니다.

```xml
<terminal_history>
<command index="1" command="pytest -q" exit_code="1" cwd="~/project" duration="1.4s" at="2026-10-21T23:30:51+09:00">
<output>
F                                                                        [100%]
...
1 failed in 0.04s
</output>
</command>
</terminal_history>
```

| 속성 | 뜻 |
| --- | --- |
| `index` | 1은 가장 최근 명령, 3은 세 번 전의 명령. |
| `command` | 명령줄. |
| `exit_code` | 알 수 있을 때만 있습니다. |
| `cwd` | 명령을 실행한 폴더. |
| `hostname`, `shell` | `cdg term --context`를 쓸 때만 있습니다. |
| `duration` | 1초 미만은 `ms`, 그 이상은 초 단위. |
| `at` | UTC 오프셋이 붙은 시작 시각. |
| `truncated="true"` | 출력을 64 KB로 잘랐고, 가운데가 표시 줄로 바뀌었습니다. |
| `output_available="false"` | 셸 히스토리에서 읽어 온 명령이라 출력이 없습니다. |

출력을 일부러 저장하지 않은 경우(예를 들어 인증 정보를 출력할 수 있는 명령)에는 `<command>` 안에 `<note>`가 붙습니다. 기록이 하나도 없으면 블록은 `<terminal_history />`입니다.

## 묶음: pack

pack은 순서가 정해져 있습니다. 내 요청 문구, 실패한 명령, diff, 파일들, 그리고 예산에 맞지 않은 것이 있으면 `<omitted>` 목록 순입니다.

```xml
<context_pack project="project">
<terminal_history>
<command index="1" command="pytest -q" exit_code="1" cwd="~/project" duration="1.4s" at="2026-10-21T23:30:51+09:00">
<output>
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
</output>
</command>
</terminal_history>
<changes source="uncommitted changes (git)">
diff --git a/prices.py b/prices.py
index 3d691df..9c00411 100644
--- a/prices.py
+++ b/prices.py
@@ -1,2 +1,19 @@
-def apply_discount(price, percent):
-    return price * (1 - percent / 100)
+TAX_RATE = 0.1
+
+
+def apply_discount(price: float, percent: float) -> float:
+    """Price after taking off `percent` percent."""
+    return price * (1 - percent)
+
+
+class Cart:
+    """Items and the discount that applies to all of them."""
+
+    def __init__(self, percent: float = 0):
+        self.items = []
+        self.percent = percent
+
+    def total(self) -> float:
+        """Sum after discount and tax."""
+        subtotal = sum(apply_discount(p, self.percent) for p in self.items)
+        return subtotal * (1 + TAX_RATE)
</changes>
<file path="test_prices.py" reason="line 5 in failing output">
1: from prices import apply_discount
2: 
3: 
4: def test_ten_percent():
5:     assert apply_discount(200, 10) == 180
</file>
<file path="prices.py" reason="called from failing test test_prices.py">
 1: TAX_RATE = 0.1
 2: 
 3: 
 4: def apply_discount(price: float, percent: float) -> float:
 5:     """Price after taking off `percent` percent."""
 6:     return price * (1 - percent)
 7: 
 8: 
 9: class Cart:
10:     """Items and the discount that applies to all of them."""
11: 
12:     def __init__(self, percent: float = 0):
13:         self.items = []
14:         self.percent = percent
15: 
16:     def total(self) -> float:
17:         """Sum after discount and tax."""
18:         subtotal = sum(apply_discount(p, self.percent) for p in self.items)
19:         return subtotal * (1 + TAX_RATE)
</file>
<file path="legacy/discount.m" reason="changed">
1: function p = discount(price, percent)
2: % Price after taking off percent percent.
3: p = price * (1 - percent / 100);
4: 
5: %% Apply
</file>
</context_pack>
```

`<file>`마다 `reason`이 붙습니다. `requested`, `changed`, `line 5 in failing output`, `called from failing test ...`, `tests prices.py`, `imported by ...` 중 하나입니다. pack 안의 파일에는 항상 줄 번호가 붙습니다.

예산에 맞지 않은 파일과, 내가 지정했지만 인증 정보라서 뺀 파일은 말없이 잘리지 않고 이름과 이유가 목록에 남습니다. 아래는 `cdg pack small.py notes.py .env -b 1000`의 결과입니다.

```xml
<context_pack project="ex">
<file path="small.py" reason="requested">
1: VALUE = 1
</file>
<omitted>
.env: may contain secrets
notes.py: over budget (~3,170 tokens)
</omitted>
</context_pack>
```

## 파이프로 보낼 때와 터미널에서

| | 터미널 | 파이프 또는 리다이렉트 |
| --- | --- | --- |
| 블록 | 클립보드에 복사. `-s`를 붙일 때만 출력. | stdout으로 출력. |
| 안내 메시지 | 요약과 함께 출력(크기, 어떤 파일인지). | stderr로 출력되어, 파이프에는 블록만 갑니다. |
| 클립보드 | 사용. | `--copy`를 붙이지 않으면 건드리지 않음. |
| 진행 표시 | 작업하는 동안 표시. | 표시하지 않음. |

그래서 `cdg scan > ctx.xml`로 깨끗한 파일을 얻을 수 있고, 에이전트의 셸은 내 클립보드를 건드리지 않고 텍스트만 받습니다. `-o 파일`은 어느 경우에나 블록을 끝에 줄바꿈을 붙여 파일로 저장합니다.

대답할 사람이 없는 상태에서 큰 `scan`을 실행하면 기다리지 않고 힌트와 함께 멈춥니다. 전부 가져오려면 `-y`를 붙입니다.
