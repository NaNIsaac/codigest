# 명령어 레퍼런스

`cdg`는 `codigest`의 줄임말입니다. `cdg --help`는 명령을 아래와 같은 묶음으로 보여 주고, `cdg <command> --help`는 명령 하나의 옵션을 보여 줍니다.

| 묶음 | 명령 |
| --- | --- |
| 코드 공유 | [`scan`](#cdg-scan), [`digest`](#cdg-digest), [`tree`](#cdg-tree) |
| 변경 공유 | [`mark`](#cdg-mark), [`diff`](#cdg-diff), [`semdiff`](#cdg-semdiff) |
| 실패한 명령 공유 | [`cap`](#cdg-cap), [`pack`](#cdg-pack), [`term`](#cdg-term) |
| 에이전트 | [`mcp`](#cdg-mcp) |
| 설정 | [`status`](#cdg-status), [`init`](#cdg-init), [`clean`](#cdg-clean), [`shell-help`](#cdg-shell-help) |

`cdg --version`(`-v`)은 설치된 버전을 출력합니다.

## 출력 옵션 {#output-options}

대부분의 명령은 텍스트 블록 하나를 만들고, 그 블록이 어디로 갈지는 같은 옵션으로 정합니다.

- 터미널에서는 블록이 클립보드에 복사되고, 복사했다는 안내가 나옵니다.
- 출력이 파이프나 파일로 갈 때는 (`cdg scan > ctx.xml`, `cdg digest | less`, 스크립트, 코딩 에이전트의 셸) 블록을 그대로 출력하고, 클립보드는 건드리지 않으며, 안내 메시지는 stderr로 나갑니다. 그래도 복사하려면 `--copy`를 붙입니다.
- `-s`를 붙이면 터미널에서도 블록을 출력합니다. `-o 파일`은 파일로도 저장합니다.
- SSH 접속 중이면 (`SSH_CONNECTION`, `SSH_CLIENT`, `SSH_TTY` 중 하나가 있으면) 원격 머신의 클립보드 대신 OSC 52 이스케이프 시퀀스로 지금 키보드를 쓰는 머신의 클립보드에 넣습니다. 터미널이 OSC 52를 받아야 하고, tmux 안이라면 `set -g set-clipboard on`을 추가합니다. `CODIGEST_CLIPBOARD=osc52`로 강제하고 (예: `docker exec`) `CODIGEST_CLIPBOARD=system`으로 끕니다.

| 옵션 | 뜻 |
| --- | --- |
| `-c`, `--copy`, `--no-copy` | 클립보드 복사를 강제하거나 끕니다. |
| `-s`, `--stdout` | 결과를 화면에도 출력합니다. 출력이 파이프로 갈 때는 기본으로 켜집니다. |
| `-o`, `--output` | 결과를 이 파일에도 저장합니다. |
| `-m`, `--message` | 질문이나 요청을 결과에 `<user_instruction>` 블록으로 넣습니다. |
| `-r`, `--resolve` | 대상이 import하는 프로젝트 파일을 재귀적으로 함께 담습니다. |
| `-a`, `--all` | `.codigest/config.toml`의 필터를 무시합니다. |

아래에서 명령마다 받는 옵션을 나열합니다. 모든 명령이 모든 공통 옵션을 받는 것은 아닙니다.

## 출력 형식 {#formats}

`scan`과 `term`은 따로 지정하지 않으면 XML 블록을 출력합니다. `--format markdown`은 Markdown을 렌더링하는 채팅용입니다. `scan`은 요청 문구, 구조, 그리고 파일마다 제목과 확장자에 맞는 언어를 붙인 코드 블록을 만들고, `term`은 명령마다 제목, 세부 정보 목록, 그리고 명령과 출력을 담은 `console` 블록 하나를 만듭니다. 코드 펜스는 내용 속 백틱 연속보다 항상 깁니다. `--format json`은 스크립트용입니다. 블록 캐시에는 XML이 들어 있으므로, 다른 형식은 파일을 새로 읽습니다.

`scan --format json`은 `project`, `instruction`, `structure`, `files`(`path`, 범위일 때 `lines`, `content`)를 가진 객체입니다. `term --format json`은 배열이며, 항목마다 최근부터 거꾸로 센 `index`, `command`, `exit_code`, `cwd`, `started_at`, `duration_ms`, `output`, `output_available`, `truncated`가 있고, `--context`를 주면 `hostname`과 `shell`, 메모가 있으면 `note`가 더해집니다.

```bash
cdg scan src/app.py:40-60 --format markdown -m "Why does this loop twice?"
cdg term -n 3 --failed --format json
```

## 프리셋 {#presets}

프리셋은 이름을 붙여 저장한 옵션 묶음입니다. `--preset NAME`은 모든 명령에서 쓸 수 있고, 저장해 둔 옵션을 명령 이름 바로 뒤에 넣으므로 명령줄에 직접 쓴 옵션이 이깁니다. 옵션을 해석하기 전에 펼쳐지며 `--help`에는 나오지 않습니다.

```toml
# Windows는 %APPDATA%\codigest\config.toml, 그 밖에는 ~/.config/codigest/config.toml,
# 또는 프로젝트의 .codigest/config.toml (같은 이름의 사용자 프리셋을 대신함)
[presets.term.debug]
format = "markdown"
count = 3
failed = true
```

```bash
cdg term --preset debug       # 실패한 최근 명령 3개를 Markdown으로
cdg term --preset debug -n 1  # 같은 설정으로 마지막 하나만
```

키는 옵션 이름에서 `-`를 `_`로 바꾼 것입니다(`no_copy = true`는 `--no-copy`). `true`는 플래그를 넣고 `false`는 넣지 않으며, 목록은 쉼표로 이어 붙입니다. 없는 프리셋을 고르면 종료 코드 2로 멈추고, 정의할 파일 위치를 알려 줍니다. 같은 옵션을 두 번 쓰면 나중 것이 이깁니다.

## `cdg scan`

프로젝트 전체, 또는 지정한 경로만 하나의 블록으로 묶습니다.

```console
cdg scan [OPTIONS] [targets]...
```

대상을 주지 않으면 무시되지 않는 모든 파일을 담습니다. `.gitignore`, [설정 필터](/ko/configuration), [항상 제외하는 폴더](/ko/security#what-is-never-included)를 따르며, 파일 트리와 각 파일의 내용이 들어갑니다.

대상은 파일, 폴더, 또는 줄 범위를 붙인 파일입니다. `src/app.py:40-80`은 40~80번째 줄, `src/app.py:42`는 한 줄입니다. 범위로 지정한 내용은 줄 번호가 붙고, `<file>` 블록에 `lines` 속성이 생깁니다. 같은 파일을 범위를 달리해 여러 번 지정할 수도 있습니다.

| 옵션 | 뜻 |
| --- | --- |
| `-o`, `--output` | 결과를 이 파일에도 저장합니다. |
| `-c`, `--copy`, `--no-copy` | 클립보드에 복사합니다 (기본: 터미널에서는 복사, 파이프일 때는 복사하지 않음). |
| `-s`, `--stdout` | 결과를 화면에도 출력합니다. |
| `-a`, `--all` | 설정 필터를 무시합니다. |
| `-m`, `--message` | 결과에 요청 문구를 넣습니다. |
| `-l`, `--lines` | 코드 블록에 줄 번호를 붙입니다. |
| `-y`, `--yes` | 크기 확인을 건너뜁니다. |
| `-r`, `--resolve` | 대상이 import하는 파일을 재귀적으로 담습니다. |
| `--format` | `xml`(기본), `markdown`, `json`. [출력 형식](#formats)을 참고하세요. |
| `--github` | 대상 `OWNER/REPO`를 GitHub에서 읽습니다 (github.com URL이면 자동으로 켜짐). |
| `--ref` | GitHub: 브랜치, 태그 또는 커밋 (기본: 저장소의 기본 브랜치). |
| `--path` | GitHub: 저장소의 이 경로만 읽습니다. 여러 번 줄 수 있고, 줄 범위는 `FILE:40-60`입니다. |
| `--auth` | GitHub: 토큰(`GH_TOKEN`, `GITHUB_TOKEN` 또는 `gh`)을 보내 비공개 저장소를 읽습니다. |
| `--max-file-size` | GitHub: 이보다 큰 파일은 내려받지 않고 건너뛴 파일로 표시합니다 (기본: `1MB`). |

GitHub 저장소(`OWNER/REPO --github` 또는 그 URL)도 복제하지 않고 같은 방식으로 읽습니다.

추정 토큰이 3만을 넘거나 파일이 100개를 넘으면 `scan`은 계속할지 묻습니다. 대답할 사람이 없을 때(stdin이 터미널이 아닐 때)는 기다리지 않고 힌트와 함께 종료 코드 1로 멈춥니다. 전부 가져오려면 `-y`를 붙입니다. 빼 놓은 인증 정보 파일이 있으면 그 이름을 알려 주는 줄이 나옵니다.

```bash
cdg scan                      # 프로젝트 전체
cdg scan src/app.py -r        # 파일 하나와, 그 파일이 import하는 프로젝트 파일
cdg scan src/billing/ docs/   # 폴더나 파일 여러 개
cdg scan src/app.py -l        # 줄 번호를 붙여서
cdg scan src/app.py:40-80     # 40~80번째 줄만
cdg scan OWNER/REPO --github --ref v1.2 --path src/app.py:40-60   # GitHub 저장소 파일의 일부 줄
```

## `cdg digest`

Python과 MATLAB 파일의 개요를 줄 번호가 붙은 선언과 docstring으로 만들고, 본문은 뺍니다.

```console
cdg digest [OPTIONS] [targets]...
```

Python 파일(`.py`, `.pyi`)은 함수, 클래스, 최상위 변수를 나열합니다. MATLAB 파일(`.m`)은 함수와 `%%` 구역을 나열합니다. 항목마다 시작하는 줄 번호와 docstring(또는 도움말 주석)의 첫 줄이 붙습니다. 클래스 속성과 dataclass 필드는 값과 함께 나열하고, 길면 줄입니다. 다른 종류의 파일은 digest에 들어가지 않지만 파일 트리에는 나옵니다.

| 옵션 | 뜻 |
| --- | --- |
| `-c`, `--copy`, `--no-copy` | 클립보드에 복사합니다 (기본: 터미널에서는 복사, 파이프일 때는 복사하지 않음). |
| `-o`, `--output` | 결과를 이 파일에도 저장합니다. |
| `-s`, `--stdout` | 결과를 화면에도 출력합니다. |
| `-m`, `--message` | 결과에 요청 문구를 넣습니다. |
| `-r`, `--resolve` | 대상이 import하는 파일을 재귀적으로 담습니다. |

```bash
cdg digest                # 프로젝트 전체
cdg digest src/billing/   # 폴더 하나만
```

## `cdg tree`

프로젝트 파일 트리를 출력합니다.

```console
cdg tree [OPTIONS] [target]
```

터미널에서는 파일 크기와 함께 트리를 그려 줍니다. `-L`, `-s`, 또는 파이프를 쓰면 일반 텍스트가 되고, `--copy`와 `-o`는 이 텍스트를 받습니다. 다른 명령과 달리 `tree`는 `--copy`를 붙이지 않으면 복사하지 않습니다.

| 옵션 | 뜻 |
| --- | --- |
| `-c`, `--copy`, `--no-copy` | 클립보드에 복사합니다 (기본: 복사하지 않음). |
| `-o`, `--output` | 결과를 이 파일에도 저장합니다. |
| `-s`, `--stdout` | 결과를 화면에도 출력합니다. |
| `-a`, `--all` | 설정 필터를 무시합니다. |
| `-r`, `--resolve` | import를 재귀적으로 따라갑니다. |
| `-L`, `--depth` | 이 단계까지만 보여 주고, 더 깊은 폴더는 파일 개수로 줄입니다. 최솟값은 1입니다. |

```console
$ cdg tree -L 1
├── .gitignore
├── legacy/ (1 file)
├── prices.py
└── test_prices.py
```

## `cdg mark`

`diff`, `semdiff`, `pack`이 비교할 기준점을 잡습니다.

```console
cdg mark [target]
```

기준점은 `.codigest/`에 보관하는 파일들의 비공개 사본입니다. Git 기록은 건드리지 않습니다. `mark`를 다시 실행하면 기준점이 옮겨 갑니다. 옵션은 없습니다.

```console
$ cdg mark
Baseline set. `cdg diff` now shows changes from here.
```

## `cdg diff`

기준점 이후의 텍스트 diff를 보여 줍니다.

```console
cdg diff [OPTIONS] [target]
```

기준점이 없으면 `cdg mark`를 먼저 실행하라고 안내하고 종료 코드 1로 끝납니다. 인증 정보 파일의 변경 내용은 안내 문구로 바뀌고, 토큰은 가려집니다.

| 옵션 | 뜻 |
| --- | --- |
| `-c`, `--copy`, `--no-copy` | 클립보드에 복사합니다 (기본: 터미널에서는 복사, 파이프일 때는 복사하지 않음). |
| `-o`, `--output` | 결과를 이 파일에도 저장합니다. |
| `-s`, `--stdout` | 결과를 화면에도 출력합니다. |
| `-m`, `--message` | 결과에 요청 문구를 넣습니다. |
| `-r`, `--resolve` | import를 재귀적으로 따라갑니다. |

```console
$ cdg diff
Checking changes since last mark (1 second ago)...
Changes Detected! (877 chars)
<changes project="project" source="since cdg mark (1 second ago)">
diff --git a/prices.py b/prices.py
index 9c00411..34ef031 100644
--- a/prices.py
+++ b/prices.py
@@ -3,7 +3,7 @@ TAX_RATE = 0.1
 
 def apply_discount(price: float, percent: float) -> float:
     """Price after taking off `percent` percent."""
-    return price * (1 - percent)
+    return price * (1 - percent / 100)
 
 
 class Cart:
@@ -13,7 +13,11 @@ class Cart:
         self.items = []
         self.percent = percent
 
-    def total(self) -> float:
+    def total(self, tax: bool = True) -> float:
         """Sum after discount and tax."""
         subtotal = sum(apply_discount(p, self.percent) for p in self.items)
-        return subtotal * (1 + TAX_RATE)
+        return subtotal * (1 + TAX_RATE) if tax else subtotal
+
+
+def tax_of(amount: float) -> float:
+    return amount * TAX_RATE
</changes>
```

## `cdg semdiff`

기준점 이후 어떤 Python 함수와 클래스가 바뀌었는지 보여 줍니다.

```console
cdg semdiff [OPTIONS] [target]
```

바뀐 `.py`, `.pyi` 파일만 비교합니다. 각 줄은 표지로 시작합니다.

| 표지 | 뜻 |
| --- | --- |
| `[ADDED]` | 함수, 클래스, 메서드, 변수가 새로 생겼습니다. |
| `[REMOVED]` | 지워졌습니다. |
| `[SIGNATURE]` | 시그니처가 바뀌었습니다. `::` 뒤에 바뀌기 전과 후가 나옵니다. |
| `[LOGIC]` | 시그니처는 같고 본문이 바뀌었습니다. |

기준점 이후 새로 생기거나 지워진 파일은 경로 뒤에 `(NEW)`, `(DELETED)`가 붙습니다.

| 옵션 | 뜻 |
| --- | --- |
| `-c`, `--copy`, `--no-copy` | 클립보드에 복사합니다 (기본: 터미널에서는 복사, 파이프일 때는 복사하지 않음). |
| `-o`, `--output` | 결과를 이 파일에도 저장합니다. |
| `-s`, `--stdout` | 결과를 화면에도 출력합니다. |
| `-m`, `--message` | 결과에 요청 문구를 넣습니다. |
| `-r`, `--resolve` | import를 재귀적으로 따라갑니다. |

```console
$ cdg semdiff
Analyzing structural changes since (1 second ago)...
SemDiff Generated! (~70 Tokens)
<semantic_diff project="project" source="since cdg mark (1 second ago)">
<file path="prices.py">
[SIGNATURE] 16: method Cart.total :: (self) -> float -> (self, tax: bool=True) -> float
[LOGIC]     4: function apply_discount
[ADDED]     22: function tax_of
</file>
</semantic_diff>
```

## `cdg cap`

명령을 평소처럼 실행하되, 그 출력을 `pack`과 `term`이 쓸 수 있게 남겨 둡니다.

```console
cdg cap [출력 옵션] [--] COMMAND...
```

명령 뒤에 오는 것은 모두 그 명령의 몫입니다. 명령의 플래그가 그대로 넘어가도록 명령 앞에 `--`를 붙이세요. 출력은 `cap` 없이 실행할 때와 똑같이 실시간으로 나오고, stdout과 stderr는 섞인 하나의 흐름으로 기록됩니다. `cap`은 실행한 명령의 종료 코드로 끝나므로 스크립트에서 명령 앞에 둘 수 있습니다.

셸 문자(`|`, `&`, `>`, `<`, `;`, `$`, `` ` ``, `*`, `?`, `(`, `)`)가 든 인자 하나는 셸을 거쳐 실행됩니다. 파이프라인은 이렇게 동작합니다: `cdg cap "npm run build | tail -20"`. 이때 쓰는 셸은 Windows에서는 PowerShell(설치되어 있으면), 그 밖에는 `$SHELL` 또는 bash입니다. 다른 셸을 쓰려면 `CODIGEST_SHELL`을 설정하세요.

PowerShell에서는 스크립트 블록도 됩니다: `cdg cap { Get-ChildItem | Sort-Object Length }`. PowerShell은 `pwsh { ... }`에 넘길 때와 같은 방식으로 블록을 넘기고, `cap`은 이를 `pwsh -NoProfile`로 실행합니다. 그래서 지금 세션의 변수와 함수는 블록 안에서 쓸 수 없습니다. 종료 코드는 `pwsh -Command`를 따릅니다. `exit`에 준 숫자, 그렇지 않으면 마지막 문장이 실패했을 때 1, 아니면 0입니다.

명령 앞에 출력 옵션을 쓰면 `cap`은 명령이 끝난 뒤 그 실행을 `cdg term`이 줄 것과 같은 블록으로 넘깁니다: `-c`/`--copy`, `--no-copy`, `-s`/`--stdout`, `-o`/`--output 파일`, `--format xml|markdown|json` ([출력 옵션](#output-options) 참고). 옵션이 없으면 기록만 합니다. 스크립트 블록 앞에서는 긴 형태 `--copy`, `--output`을 쓰세요. PowerShell이 `-c`와 `-o`를 자기 `-Command`, `-OutputFormat`으로 읽습니다.

| 종료 코드 | 언제 |
| --- | --- |
| 명령 자신의 코드 | 명령이 실행되었을 때 |
| 2 | 명령을 주지 않았거나 출력 옵션이 잘못되었을 때 |
| 127 | 프로그램을 찾지 못했을 때 |

기록은 프로젝트별이 아니라 컴퓨터 한 곳에 모읍니다. [터미널 기록](/ko/terminal-capture)을 참고하세요.

```bash
cdg cap -- pytest -q
cdg cap "npm run build | tail -20"
cdg cap --copy -- pytest -q          # 실행한 뒤 결과를 채팅용으로 복사
cdg cap -s --format markdown { Get-ChildItem | Sort-Object Length -Descending | Select-Object -First 5 }
```

## `cdg pack`

마지막 실패, 그 실패가 가리키는 파일, 내 변경 사항을 하나로 묶습니다.

```console
cdg pack [OPTIONS] [targets]...
```

그냥 `cdg pack`을 실행하면 정해진 순서로 토큰 예산을 채웁니다. 마지막으로 실패한 명령이 가리키는 파일, 실패한 테스트가 호출하는 코드, 내가 바꾼 파일, 그 파일을 import하는 테스트, 그리고 그들이 import하는 프로젝트 파일 순입니다. 파일을 직접 지정하면 그 파일만 묶습니다. 전체 순서는 [`pack`이 파일을 고르는 기준](/ko/getting-started#how-pack-decides)에 있습니다. 묶을 것이 없으면 종료 코드 1로 끝납니다.

| 옵션 | 뜻 |
| --- | --- |
| `-m`, `--message` | 결과에 요청 문구를 넣습니다. |
| `-b`, `--budget` | 토큰 예산입니다. 한 토큰을 네 글자로 어림합니다. 기본값 30000, 최솟값 1000. |
| `--with` | 자동으로 넣을 항목. `failure`, `changes`, `tests`, `imports` 중 쉼표로 구분하거나 `none`. |
| `-c`, `--copy`, `--no-copy` | 클립보드에 복사합니다 (기본: 터미널에서는 복사, 파이프일 때는 복사하지 않음). |
| `-s`, `--stdout` | 결과를 화면에도 출력합니다. |
| `-o`, `--output` | 결과를 이 파일에도 저장합니다. |

`--with`를 주지 않으면 네 항목이 모두 켜지고, 파일을 지정하면 모두 꺼집니다. 실패한 명령과 diff는 해당 항목이 켜져 있으면 항상 들어갑니다. stderr로 나오는 요약에는 파일마다 넣은 이유가 나오고, 뺀 파일 이름도 나옵니다.

```console
$ cdg pack prices.py --with tests
Packed ~209 / 30,000 tokens
  auto     tests (--with failure,changes,tests,imports adds more)
  + prices.py       requested
  + test_prices.py  tests prices.py
```

```bash
cdg pack                             # 여기서 마지막 실패 + 내 변경
cdg pack src/prices.py               # 이 파일만
cdg pack src/prices.py --with tests  # 이 파일과 그 테스트
cdg pack --with changes              # 실패는 빼고 내 변경만
cdg pack -m "fix the discount bug"   # 요청 문구 추가
```

## `cdg term`

최근에 기록된 명령과 그 출력을 보여 줍니다.

```console
cdg term [OPTIONS] [COMMAND]
```

하위 명령 없이 실행하면 가장 최근 기록을 출력합니다. 기록된 것이 없으면 셸의 히스토리(PowerShell, bash, zsh, fish)에서 읽어 옵니다. 이때는 출력 없이 명령만 나옵니다.

| 옵션 | 뜻 |
| --- | --- |
| `-n`, `--count` | 가져올 최근 명령의 수. 기본값 1. |
| `-i`, `--index` | 가장 최근부터 거꾸로 세어 정확히 하나만 고릅니다 (1 = 마지막). |
| `-f`, `--failed` | 종료 코드가 0이 아닌 명령만. |
| `--here` | 현재 폴더 또는 그 아래에서 실행한 명령만. |
| `--history-only` | Codigest 기록 대신 셸 히스토리를 읽습니다 (출력 없음). |
| `--context` | 기록된 PC 이름과 셸 정보를 결과에 넣습니다. |
| `--format` | `xml`(기본), `markdown`, `json`. [출력 형식](#formats)을 참고하세요. |
| `-c`, `--copy`, `--no-copy` | 클립보드에 복사합니다 (기본: 터미널에서는 복사, 파이프일 때는 복사하지 않음). |
| `-s`, `--stdout` | 결과를 화면에도 출력합니다. |
| `-o`, `--output` | 결과를 이 파일에도 저장합니다. |

```bash
cdg term              # 마지막 명령과 출력
cdg term -n 3         # 최근 3개
cdg term -f           # 마지막으로 실패한 명령
cdg term --here -n 3  # 이 프로젝트에서 실행한 최근 3개
cdg term -i 2         # 두 번 전에 실행한 명령 하나만
cdg term --context    # PC 이름과 셸 정보 추가
```

#### `--raw`, 바이너리에만 있음 {#term-raw}

빌드된 바이너리(Rust 빌드)에는 여기에 옵션이 하나 더 있습니다. `term --raw`는 명령 하나의 저장된 출력을 태그 없이 출력합니다. Python 패키지에는 없습니다. [Rust 포팅](/ko/rust-port)을 참고하세요.

### `cdg term install`

셸에 훅을 설치해 모든 명령의 출력을 자동으로 기록합니다.

| 옵션 | 뜻 |
| --- | --- |
| `--shell` | `powershell`, `bash`, `zsh`, `all` 중 하나. 기본값: 설치되어 있는 셸 전부. |

### `cdg term uninstall`

셸 시작 파일에서 기록용 훅을 제거합니다.

| 옵션 | 뜻 |
| --- | --- |
| `--shell` | `powershell`, `bash`, `zsh`, `all` 중 하나. 기본값: 설치되어 있는 셸 전부. |

### `cdg term clear`

저장된 터미널 기록을 모두 지웁니다. 옵션은 없습니다.

### `cdg term path`

터미널 기록이 저장된 위치를 보여 줍니다. 옵션은 없습니다.

### `cdg term ingest`

기록 하나를 저장합니다. 셸 훅이 호출하는 명령으로, `--help`에는 나오지 않으며 직접 실행하는 용도가 아닙니다. PowerShell은 stdin으로 JSON을 넘기고, bash와 zsh는 임시 파일과 아래 옵션을 넘깁니다.

| 옵션 | 뜻 |
| --- | --- |
| `--file` | 명령, NUL 문자, 출력 순서로 담긴 파일. 저장한 뒤 지웁니다. |
| `--exit-code` | 명령의 종료 코드. |
| `--cwd` | 명령을 실행한 폴더. |
| `--shell` | 명령을 실행한 셸. |
| `--start` | 시작 시각(epoch 초). |
| `--end` | 끝난 시각(epoch 초). |

저장에 실패하면 오류가 기록 옆의 `ingest-errors.log`에 남고, `cdg term`이 이를 알려 줍니다.

## `cdg mcp`

이 도구들을 AI 에이전트에게 MCP(stdio)로 제공합니다.

```console
cdg mcp
```

옵션은 없으며, stdin과 stdout으로 줄 단위 JSON-RPC를 주고받습니다. 등록 방법과 제공하는 도구는 [MCP 연동](/ko/mcp)을 참고하세요.

## `cdg status`

지금 `pack`이 무엇을 쓸지 보여 줍니다: 마지막 실패, 내 변경, 기준점.

```console
cdg status [target]
```

옵션은 없고, 아무것도 쓰지 않습니다.

```console
$ cdg status
project  ~/project
  failure   pytest -q (exit 1, just now)
  changes   1 file, since cdg mark (2 seconds ago)
            prices.py
  baseline  set 2 seconds ago (cdg mark moves it)
  files     4 considered
```

## `cdg init`

`.codigest/config.toml`(파일 필터)을 만듭니다.

```console
cdg init [OPTIONS] [target]
```

설정 파일을 쓰고, `.codigest/`에 자체 `.gitignore`를 두어 Git에 보이지 않게 하며, 프로젝트의 `.gitignore`에 `.codigest/`를 추가합니다. 기준점은 잡지 않으므로 그것은 `cdg mark`로 합니다. 다른 명령을 쓰기 위해 `init`이 꼭 필요한 것은 아닙니다.

| 옵션 | 뜻 |
| --- | --- |
| `-f`, `--force` | 이미 있는 설정을 덮어씁니다. |

## `cdg clean`

`.codigest/`(설정, 기준점, 캐시)를 지웁니다.

```console
cdg clean [OPTIONS] [target]
```

따로 지정하지 않으면 지우기 전에 먼저 물어봅니다. 소스 파일은 건드리지 않습니다.

| 옵션 | 뜻 |
| --- | --- |
| `-y`, `--yes`, `-f`, `--force` | 확인 질문을 건너뜁니다. |

## `cdg shell-help`

터미널 자동 기록을 켜지 않고도 PowerShell `Get-Help`가 codigest를 찾게 합니다.

PowerShell의 `help codigest`는 `codigest --help`와 별개의 명령입니다. `shell-help install`은 PowerShell 프로필에 작은 연결 함수를 추가하고, `uninstall`은 그것을 제거합니다. 다른 `help` 명령은 바뀌지 않고, 명령 자동 기록도 켜지지 않습니다. 실행한 뒤에는 PowerShell 새 창을 여세요. 두 하위 명령 모두 옵션은 없습니다.

### `cdg shell-help install`

codigest의 PowerShell 도움말을 등록합니다 (선택 사항).

### `cdg shell-help uninstall`

Codigest의 PowerShell 도움말 연결만 제거합니다.
