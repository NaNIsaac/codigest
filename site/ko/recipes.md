# 활용 예시

상황에서 붙여 넣기까지 이어지는 작업 흐름을 모았습니다. 예시는 [빠른 시작](/ko/getting-started)의 작은 프로젝트(`prices.py`, `test_prices.py`, `legacy/` 폴더)를 씁니다.

## 테스트가 실패하는데 이유를 모를 때 {#a-test-fails-and-you-do-not-know-why}

테스트를 `cap`으로 실행하고 pack을 만듭니다.

```bash
cdg cap -- pytest -q
cdg pack -m "Why does test_ten_percent fail?"
```

`cap`의 출력은 평소 `pytest -q`와 똑같이 보입니다. `pack`은 실패한 명령과 출력, 아직 커밋하지 않은 변경, traceback이 가리킨 테스트 파일, 그 테스트가 호출하는 코드를 모아 줍니다.

```console
$ cdg pack
Packed ~689 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py     line 5 in failing output
  + prices.py          called from failing test test_prices.py
  + legacy/discount.m  changed
```

이것을 채팅창에 붙여 넣으면 됩니다. 버그가 이미 커밋되어 diff에 아무것도 없어도, 실패한 테스트가 호출하므로 `prices.py`는 들어갑니다. 코드를 고친 뒤 `cdg cap -- pytest -q`를 다시 실행하세요. 같은 명령이 통과하면 그 실패는 더 이상 `pack`이 쓰지 않습니다.

pack이 생각보다 크면 예산을 줄이거나(`-b 8000`) 필요한 것만 지정하세요: `cdg pack prices.py test_prices.py`.

## 긴 실패 출력에서 필요한 부분만

출력이 수천 줄인 명령은 pack에 앞 80줄과 뒤 40줄만 남습니다. 처음 몇 개의 실패와 요약이 있는 곳입니다. 출력에 이름만 나왔고 내가 고치지 않은 파일은, 출력이 가리킨 곳마다 앞뒤 열 줄씩만 들어갑니다. 어느 쪽이든 무엇을 뺄지 내가 고르지 않아도 pack이 작게 유지됩니다.

기록된 것 전부를 보려면 `cdg term -f`를 쓰세요.

## mark와 semdiff로 리팩터링 검토하기 {#review-a-refactor-with-mark-and-semdiff}

작업을 시작하기 전에 기준점을 잡습니다. Git을 건드리지 않으므로 변경이 쌓인 상태나 저장소가 없는 폴더에서도 쓸 수 있습니다.

```bash
cdg mark
# ... 리팩터링 ...
cdg semdiff -m "Review these signature changes for callers I may have missed"
```

`semdiff`는 "내가 실제로 무엇을 바꿨는가"를 함수와 클래스 단위로 알려 줍니다.

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

`[SIGNATURE]` 줄은 호출하는 쪽이 깨질 수 있는 곳이라, 보통 다른 의견을 들어 볼 만한 부분입니다. 모델이 본문을 보여 달라고 하면 그 부분만 보내세요: `cdg scan prices.py:4-6`. 텍스트 변경 전체가 필요하면 `cdg diff`를 씁니다.

`semdiff`는 Python을 읽습니다. 다른 언어는 `cdg diff`를 쓰세요. `cdg mark`를 다시 실행하면 기준점이 지금 시점으로 옮겨 갑니다.

## 폴더 하나, 또는 질문에 필요한 파일만 보여 주기 {#share-one-folder-or-one-question-s-worth-of-files}

작게 시작하고, 요청이 있을 때만 넓혀 갑니다.

```bash
cdg tree -L 2                       # 프로젝트의 큰 모양
cdg digest src/billing/             # 무엇이 어디에 있는지, 본문 없이
cdg scan src/billing/invoice.py     # 질문과 관련된 파일
cdg scan src/billing/invoice.py -r  # ... 그리고 그 파일이 import하는 프로젝트 파일
cdg scan src/billing/ docs/ -l      # 폴더와 문서 폴더, 줄 번호 포함
```

`digest`는 첫 메시지로 좋습니다. 코드보다 훨씬 작고, 모델이 `Cart.total`처럼 이름으로 요청할 수 있습니다. 그러면 `cdg scan prices.py:16-19`로 답합니다. digest의 번호와 범위의 번호가 같으므로, 나도 모델도 "18번째 줄"이라고 말할 수 있습니다.

질문도 같은 블록에 `-m`으로 넣고, 나중을 위해 파일로 남길 수도 있습니다.

```bash
cdg scan src/billing/ -l -m "Where is the rounding done?" -o ctx.xml
```

결과가 3만 토큰이나 파일 100개를 넘으면 `scan`이 먼저 물어봅니다. 경로를 좁히거나 `cdg digest`를 쓰세요.

## 잡음과 비밀정보 빼기

`cdg init`으로 설정을 만들고, `.codigest/config.toml`을 고쳐 필요한 파일 종류만 남기고 모델이 보면 안 되는 폴더는 뺍니다.

```toml
[filter]
extensions = [".py", ".md"]
exclude_patterns = ["tests/data/", "*.lock"]
```

`.env` 파일과 키 파일은 설정과 상관없이 절대 들어가지 않습니다. [설정](/ko/configuration)과 [보안](/ko/security)을 참고하세요.

## 빌드를 기록하고 물어보기

```bash
cdg cap "npm run build | tail -20"
cdg term -f
```

`cdg term -f`는 마지막으로 실패한 명령을 복사합니다. 셸 훅(`cdg term install`)을 설치했다면 `cap`도 필요 없습니다. 빌드를 평소처럼 실행한 뒤 `cdg pack`이나 `cdg term -f`를 실행하세요.

## 에이전트가 직접 모으게 하기

명령을 실행할 수 있는 코딩 에이전트는 아무 설정도 필요 없습니다. 에이전트의 셸은 터미널이 아니므로 Codigest는 결과를 출력만 하고 클립보드는 건드리지 않습니다. 에이전트에게 맞는 순서는 다음과 같습니다.

```bash
cdg tree -L 2                      # 어디에 무엇이 있는지
cdg digest src/                    # 무엇이 어디에 정의되어 있는지, 줄 번호와 함께
cdg scan src/prices.py:16-19      # 필요한 부분만, 번호와 함께 읽기
cdg mark                           # 수정하기 전에
cdg semdiff                        # 수정한 뒤: 어떤 함수가 바뀌었는지
cdg cap -- pytest -q || cdg pack   # 테스트 실행. 실패하면 pack에 고칠 곳이 담김
```

MCP를 쓰는 에이전트는 셸 없이도 같은 결과를 도구로 요청할 수 있습니다. [MCP 연동](/ko/mcp)을 참고하세요.

## 프로젝트를 옮겨 다닐 때

기록은 프로젝트별이 아니라 컴퓨터 한 곳에 모입니다. `~/work/api`에서 기록한 실패는 `~/work/web`에서 `cdg pack`을 실행해도 쓰지 않습니다. `pack`은 현재 프로젝트에서 실행한 실패만 찾기 때문입니다. `cdg term`은 어디서 실행했든 가장 최근 명령을 보여 주고, `cdg term --here`는 현재 폴더와 그 아래로 좁힙니다.
