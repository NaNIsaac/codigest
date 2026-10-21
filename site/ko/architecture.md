# 구조

Codigest는 Python 패키지(`codigest`)이며 `codigest`와 `cdg` 두 명령으로 설치됩니다. [Typer](https://typer.tiangolo.com/) 앱이고, 명령마다 작은 모듈이 입력을 모아 공통 core를 호출한 뒤, 완성한 텍스트 블록 하나를 출력 처리기에 넘깁니다.

## 구성

```text
src/codigest/
├── main.py          명령 등록, --version, 최상위 오류 처리
├── options.py       공통 옵션 (-s, --copy, -o, -m, -r, -a)
├── commands/        명령마다 모듈 하나
│   ├── scan.py  digest.py  tree.py
│   ├── mark.py  diff.py  semdiff.py
│   ├── cap.py  term.py  pack.py  status.py
│   ├── mcp.py
│   └── init.py  clean.py  shell_help.py
├── core/
│   ├── common.py      프로젝트 루트, 설정 필터, 파일 선택
│   ├── scanner.py     트리를 돌며 .gitignore와 필터 적용
│   ├── resolver.py    import를 따라감 (Python, JS/TS 상대 경로)
│   ├── processor.py   파일 읽기, 줄 번호, 줄 범위
│   ├── outline.py     digest: 선언과 docstring (Python, MATLAB)
│   ├── semdiff.py     함수·클래스 단위 비교 (Python AST)
│   ├── shadow.py      mark, diff, semdiff 뒤의 기준점("anchor")
│   ├── git_ops.py     pack이 쓰는 읽기 전용 Git 질의
│   ├── callees.py     실패한 테스트가 호출하는 프로젝트 코드
│   ├── termstore.py   기록 저장소와 XML 출력
│   ├── runner.py      cap이 명령을 실행하며 출력을 흘려보냄
│   ├── shellhist.py   셸 히스토리 읽기
│   ├── redact.py      가리기, 거부 명령, 인증 정보 파일 이름
│   ├── prompts.py     기본 출력 포장과 prompts.toml 템플릿
│   ├── tags.py        <file> 블록과 이스케이프
│   ├── output.py      클립보드, stdout, -o, 터미널과 파이프의 차이
│   ├── cache.py       파일별 블록과 digest 캐시
│   ├── structure.py   ASCII 트리
│   └── tokenizer.py   토큰 어림값
└── shell/             hook.ps1, hook.sh, help.ps1
```

무거운 import는 지연됩니다(Python 3.15의 `lazy import`). 그래서 `cdg --help`와 짧은 명령이 빠르게 시작합니다.

## 데이터 흐름

대부분의 명령은 같은 길을 따릅니다.

1. **프로젝트 찾기.** `common.get_context`가 현재 폴더(또는 내가 지정한 경로들의 공통 부모)에서 시작해, `.codigest/` 또는 `.git/`이 처음 나오는 폴더까지 올라갑니다. 그곳이 루트입니다. 이때 `.codigest/config.toml`의 `[filter]`도 읽습니다.
2. **파일 고르기.** `scanner`가 루트를 돌며 항상 제외하는 폴더, `.gitignore`, `exclude_patterns`를 건너뛰고, `extensions`가 있으면 그것만 남깁니다. `-r`을 주면 `resolver`가 import된 파일을 더합니다. 인증 정보 파일은 여기서 빼고 기억해 두어, 명령이 뺐다는 것을 말할 수 있게 합니다.
3. **내용 만들기.** `scan`은 파일을 읽고(줄 번호나 범위 포함), `digest`는 개요를 만들고, `diff`와 `semdiff`는 기준점과 비교하고, `pack`은 파일마다 왜 필요한지로 순위를 매기고, `term`은 기록을 출력 형식으로 바꿉니다.
4. **포장하기.** `prompts`가 내용을 루트 요소로 감싸거나, `prompts.toml`의 내 템플릿에 넣습니다. `tags.file`은 `<file>` 블록마다 코드를 그대로 담습니다.
5. **출력.** `OutputHandler`가 누가 봐도 비밀인 것을 한 번 더 가리고, 복사하고, `-o`로 쓰고, stdout이 터미널인지에 따라 출력합니다.

`cap`만 다릅니다. `runner`로 프로그램을 실행하며 출력을 실시간으로 흘려보내고, `termstore`가 가리고 줄인 기록을 저장소에 더합니다. `pack`은 이 저장소를 읽은 뒤 위와 같은 흐름을 따릅니다.

## 저장되는 위치 {#where-things-are-stored}

**프로젝트 안, `.codigest/` 아래:**

| 경로 | 내용 |
| --- | --- |
| `config.toml` | 파일 필터. `cdg init`이 만듭니다. |
| `prompts.toml` | 내 템플릿. 직접 만듭니다. |
| `anchor/` | 기준점: 프로젝트 파일들의 사본. |
| `anchor/.shadow_git` | 기준점을 위한 Git 저장소. 프로젝트의 저장소와 별개입니다. |
| `cache.json` | `scan` 블록과 `digest` 요약의 캐시. 파일 내용을 키로 씁니다. 이미 `.codigest/`가 있는 프로젝트에만 씁니다. |
| `.gitignore` | `*`가 들어 있어, Git이 이 안을 전혀 보지 못합니다. |

`cdg clean`이 이 폴더를 지웁니다.

**컴퓨터 안, 프로젝트 밖:**

| 플랫폼 | 데이터 폴더 |
| --- | --- |
| Windows | `%LOCALAPPDATA%\codigest` |
| macOS, Linux | `$XDG_DATA_HOME/codigest`, 없으면 `~/.local/share/codigest` |
| 공통 | `$CODIGEST_HOME`이 설정되어 있으면 그 폴더 |

기록은 그 안의 `term/history.jsonl`에 있고, 훅이 실패한 적이 있으면 그 옆에 `term/ingest-errors.log`가 있습니다. 정확한 파일은 `cdg term path`가 출력합니다.

## 기준점(anchor)

`cdg mark`가 기준점을 잡습니다. 명령들이 지금 다루는 파일을 `.codigest/anchor/`에 복사하고, Git 디렉터리가 `.codigest/anchor/.shadow_git`인 그림자 Git 저장소에 커밋합니다. 내 프로젝트의 저장소는 이 과정에서 읽지도 쓰지도 않습니다.

- `diff`는 작업 폴더를 그곳의 마지막 커밋과 비교합니다.
- `semdiff`는 텍스트가 바뀐 파일을 찾은 뒤, 예전 버전과 새 버전의 구문 트리를 비교합니다.
- `status`와 `pack`은 기준점이 있으면 그것을 쓰고, 없으면 내 Git 작업 폴더를 씁니다.

`mark`를 다시 실행하면 새 커밋이 생기고, 그렇게 기준점이 옮겨 갑니다. 바뀐 것이 없으면 커밋하지 않습니다. 그림자 저장소는 이름이 `.git`이 아니라 `.shadow_git`이라서, 편집기가 중첩된 저장소로 인식하지 않습니다.

## 기록 저장소

한 줄에 하나씩 JSON으로 적는 파일 하나에 최근 명령 200개를 보관합니다. 출력은 명령 하나당 64 KB까지 저장하고, 넘치면 앞부분과 뒷부분만 남깁니다. 셸 훅의 기록은 백그라운드 프로세스에서 들어오므로 순서가 뒤바뀔 수 있어서, 읽을 때 시작 시각으로 정렬합니다. 파일 잠금은 없어서, 동시에 ingest가 일어나면 드물게 기록 하나가 빠질 수 있습니다.

## MCP

서버는 stdio로 주고받는 작은 JSON-RPC 루프입니다. 도구마다 일반 CLI(`python -m codigest <command> -s --no-copy`)를 하위 프로세스로 실행하므로, 에이전트는 내가 붙여 넣을 내용과 똑같은 것을 받고, stdout은 프로토콜 전용으로 깨끗하게 남습니다. [MCP 연동](/ko/mcp)을 참고하세요.

## 출력 규칙

파일 내용은 `<file>` 블록 안에 그대로 넣습니다. 파일 안에 Codigest 블록의 닫는 태그(`</file>` 같은)가 그대로 있을 때만 `&lt;/file>`로 바꿔, 블록이 중간에 끝나지 않게 합니다. [출력 형식](/ko/output-formats)을 참고하세요.
