# 빠른 시작

Codigest는 ChatGPT나 Claude, 코딩 에이전트에게 코드에 대해 물어볼 때 쓰는 명령줄 도구입니다. 프로젝트에서 내가 고른 부분을 하나의 텍스트로 묶어 주므로, 어느 채팅창에든 그대로 붙여 넣을 수 있습니다.

모델은 내가 보여 준 만큼만 압니다. 파일을 손으로 복사하면 번거롭고, 정작 중요한 파일 하나를 빼먹기 쉽고, 반대로 질문과 상관없는 코드까지 잔뜩 붙여 넣기도 쉽습니다. Codigest에서는 무엇을 보여 줄지 내가 정하고, 모으는 일은 도구가 합니다. 프로젝트 전체, 파일 하나와 그 파일이 import하는 파일들, 표시해 둔 시점 이후에 바뀐 부분, 실패한 테스트와 오류가 가리키는 코드 같은 것들입니다.

먼저 [설치](/ko/installation)하세요. 명령 이름은 `codigest`이고, 줄여서 `cdg`로도 쓸 수 있습니다. 프로젝트마다 따로 설정할 것은 없습니다.

터미널에서 실행하면 모든 명령이 결과를 클립보드에 복사합니다. `-s`를 붙이면 화면에도 출력하고, `-o 파일`을 붙이면 파일로도 저장하며, `--no-copy`를 붙이면 클립보드를 건드리지 않습니다. `-m "..."`로 질문이나 요청을 함께 넣을 수 있습니다. 출력이 파이프나 파일로 갈 때는 (`cdg scan > ctx.xml`, `cdg digest | less`, 스크립트, 코딩 에이전트의 셸) 결과를 그대로 출력하고 클립보드는 건드리지 않습니다. 그래도 복사하려면 `--copy`를 붙입니다.

## 프로젝트 보여 주기

프로젝트 폴더에서 `scan`을 실행하고 붙여 넣으면 됩니다.

```bash
cdg scan
```

클립보드에는 `.gitignore`를 따른 파일 트리와 모든 파일이 들어갑니다. 어떤 모양인지는 [출력 형식](/ko/output-formats)에서 볼 수 있습니다.

프로젝트가 크면(3만 토큰 또는 파일 100개 이상) 복사하기 전에 물어봅니다. 그럴 때는 아래처럼 일부만 보여 주세요.

## 일부만 보여 주기

```bash
cdg scan src/app.py -r       # 파일 하나와, 그 파일이 import하는 프로젝트 파일 전부
cdg scan src/billing/ docs/  # 폴더나 파일 여러 개
cdg scan src/app.py -l       # 줄 번호를 붙여서 ("42번째 줄"처럼 답할 수 있게)
cdg scan src/app.py:40-80    # 40~80번째 줄만, 번호를 붙여서 (한 줄은 src/app.py:42)
cdg digest                   # 선언과 docstring만, 훨씬 짧음
cdg digest src/billing/      # 폴더 하나만
cdg tree -L 2                # 파일 트리, 두 단계까지만
```

`digest`는 대화를 시작할 때 먼저 보여 주기 좋은 개요입니다. 이것을 본 모델은 필요한 것만 정확히 요청할 수 있고("`Cart.total`을 보여 주세요"), `cdg scan prices.py:16-19`는 그 메서드만 같은 번호로 보여 줍니다.

```console
$ cdg scan prices.py:16-19
<project_snapshot project="project">
<project_structure>
└── prices.py
</project_structure>
<source_code>
<file path="prices.py" lines="16-19">
16:     def total(self) -> float:
17:         """Sum after discount and tax."""
18:         subtotal = sum(apply_discount(p, self.percent) for p in self.items)
19:         return subtotal * (1 + TAX_RATE)
</file>
</source_code>
</project_snapshot>
```

## 바뀐 부분 보여 주기

먼저 기준점을 잡고 작업합니다.

```bash
cdg mark                     # 예: 리팩터링을 시작하기 전에
# ... 작업 ...
cdg diff                     # mark 이후의 텍스트 diff
cdg semdiff                  # Python: 어떤 함수와 클래스가 바뀌었는지
```

기준점은 `.codigest/`에 보관하는 비공개 사본입니다. Git 기록은 건드리지 않습니다.

## 실패한 명령 넘기기

명령을 `cdg cap`으로 실행합니다. 출력은 평소와 똑같이 보이고, Codigest가 사본을 남겨 둡니다. 그다음 pack을 만듭니다.

```bash
cdg cap -- pytest -q        # npm test, cargo test, go test ./..., make도 가능
cdg pack
```

```console
$ cdg pack
Packed ~689 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py     line 5 in failing output
  + prices.py          called from failing test test_prices.py
  + legacy/discount.m  changed
```

위에서부터 실패한 명령, Git에서 아직 커밋하지 않은 변경, 오류가 5번째 줄을 가리킨 `test_prices.py`, 그 테스트가 호출하는 코드가 든 `prices.py`, 내가 고친 `legacy/discount.m`을 담았다는 뜻입니다. 분량이 넘쳐 뺀 파일은 `<omitted>` 블록에 이름이 남기 때문에, 모델도 무엇이 빠졌는지 알 수 있습니다.

### `pack`이 파일을 고르는 기준 {#how-pack-decides}

그냥 `cdg pack`을 실행하면 아래 순서대로 토큰 예산(기본 30,000)을 채웁니다.

1. 이 프로젝트에서 최근 한 시간 안에 실패한 마지막 명령의 출력에 나온 파일. `File "x.py", line 7`(Python), `x.py:7`(pytest, ruff, gcc, rustc, eslint), `x.ts(7,3)`(tsc) 같은 형식을 알아봅니다. 같은 명령이 나중에 성공하면 그 실패는 더 이상 쓰지 않습니다. 내가 고치지 않았는데 출력에만 나온 파일은, 오류가 가리킨 곳마다 앞뒤 열 줄씩만 잘라서 넣고 `lines="40-60"`처럼 표시합니다. 짧은 파일은 전체를 넣습니다.
2. 실패한 Python 테스트가 호출하는 프로젝트 함수와 클래스. 전체를 넣고 이유는 `called from failing test ...`입니다. assert에서 끝나는 traceback은 테스트만 가리키므로, 버그가 이미 커밋되어 diff에 아무것도 없을 때도 테스트 대상 코드를 함께 넣기 위한 것입니다.
3. 내가 바꾼 파일: Git에서 아직 커밋하지 않은 파일과 새로 만든 파일, 또는 `cdg mark` 이후의 변경. 긴 파일은 diff hunk가 닿는 부분만 넣습니다(diff 자체는 이미 pack에 들어 있습니다).
4. 그 파일들을 import하는 테스트(`test_*.py`, `*_test.py`, `*.test.ts`, `*.spec.js` 등).
5. 그 파일들이 import하는 프로젝트 파일. Python import와 JS/TS 상대 경로 import를 따라갑니다(`./x`, `x.ts`를 가리키는 `./x.js`, `dir/index.ts`를 가리키는 `./dir`).

파일을 직접 지정하면 그 파일만 넣습니다. 자동으로 넣던 것들은 `--with`로 다시 켤 수 있습니다.

```bash
cdg pack src/prices.py                         # 이 파일만
cdg pack src/prices.py --with tests            # 이 파일과 그 테스트
cdg pack --with changes                        # 실패는 빼고 내 변경만
cdg pack --with failure,changes,tests,imports  # 그냥 `cdg pack`과 같음
```

실패한 명령과 diff는 해당 항목이 켜져 있으면 항상 넣습니다. 명령 출력이 길면 앞 80줄과 뒤 40줄(처음 몇 개의 실패와 요약)만 남기며, 전체는 `cdg term -f`로 볼 수 있습니다. 토큰 수는 네 글자를 한 토큰으로 계산한 어림값이라 실제와 조금 다를 수 있습니다.

`cdg status`를 실행하면 아무것도 만들지 않고, 지금 `pack`이 무엇을 쓸지 볼 수 있습니다.

### `cap`을 치지 않고 기록하기

`cdg term install`로 셸에 훅을 설치하면 `cdg cap`을 붙이지 않아도 모든 명령이 기록됩니다(`cdg term uninstall`로 제거). [터미널 기록](/ko/terminal-capture)을 참고하세요.

## 에이전트가 직접 가져가게 하기

코딩 에이전트를 MCP로 연결하면, 붙여 넣어 달라고 하는 대신 에이전트가 직접 가져갑니다.

```bash
claude mcp add --scope user codigest -- cdg mcp      # Claude Code
```

다른 클라이언트는 [MCP 연동](/ko/mcp)을 참고하세요. 셸 명령을 실행할 수 있는 에이전트라면 아무 설정도 필요 없습니다. 에이전트의 셸은 터미널이 아니라서 `cdg`가 결과를 그대로 출력해 주고, 내 클립보드는 건드리지 않습니다. 에이전트에게 맞는 사용 순서는 다음과 같습니다.

```bash
cdg tree -L 2                      # 어디에 무엇이 있는지
cdg digest src/                    # 무엇이 어디에 정의되어 있는지, 줄 번호와 함께
cdg scan src/prices.py:16-19      # 필요한 부분만, 같은 번호로 읽기
cdg mark                           # 수정하기 전에
cdg semdiff                        # 수정한 뒤: 어떤 함수가 바뀌었는지
cdg cap -- pytest -q || cdg pack   # 테스트 실행. 실패하면 pack에 고칠 곳이 담김
```

## 다음으로

- 모든 명령과 옵션은 [명령어 레퍼런스](/ko/commands)에서 볼 수 있습니다.
- 작업 흐름 전체는 [활용 예시](/ko/recipes)를 참고하세요.
- 무엇을 가리고 무엇을 아예 빼는지는 [보안](/ko/security)에 있습니다.
