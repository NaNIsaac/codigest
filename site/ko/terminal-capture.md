# 터미널 기록

`cdg cap`은 명령의 출력을 실시간으로 보여 주면서, 명령줄, 섞인 stdout과 stderr, 종료 코드, 작업 폴더를 기록합니다. `cdg term`은 그 기록을 꺼내 주고, `cdg pack`은 마지막으로 실패한 기록을 씁니다.

기록은 모든 프로젝트가 컴퓨터 한 곳에 모이므로, `cap`과 `term`은 저장소가 있든 없든 어느 폴더에서나 쓸 수 있습니다.

## 명령 하나 기록하기: cap

```bash
cdg cap -- pytest -q                # -- 뒤의 플래그는 실행할 명령의 몫
cdg cap "npm run build | tail -20"  # 파이프가 있으면 따옴표로 감싸기
```

출력은 `cap` 없이 실행할 때와 똑같이 보입니다. 종료 코드도 그대로 전달됩니다. 파이프라인이 어떻게 실행되는지는 [`cdg cap`](/ko/commands#cdg-cap)을 참고하세요.

## 기록 꺼내 보기: term

```bash
cdg term              # 마지막 명령과 출력
cdg term -n 3         # 최근 3개
cdg term -f           # 마지막으로 실패한 명령
cdg term --here -n 3  # 이 프로젝트에서 실행한 최근 3개
cdg term -i 2         # 두 번 전에 실행한 명령 하나만
cdg term --context    # 복사 내용에 실행 PC 이름과 셸 정보 추가 (선택)
cdg term path         # 기록 파일 위치 (CODIGEST_HOME으로 바꿀 수 있음)
cdg term clear        # 기록 모두 지우기
```

`cap` 없이 실행한 명령은 셸의 히스토리(PowerShell, bash, zsh, fish)에서 읽어 오며, 이때는 출력이 없습니다. `--history-only`를 붙이면 이 히스토리만 읽습니다.

PC 이름은 민감할 수 있어서 기본으로는 넣지 않습니다. `--context`를 붙이면 복사 내용에 `hostname`과 `shell` 속성이 붙습니다. 호스트 이름은 명령을 기록하는 시점에 저장하므로, 오래된 기록에는 없을 수 있고 그래도 `cdg term`은 문제없이 동작합니다.

## 모든 명령 기록하기: install

`cdg term install`은 설치되어 있는 셸에 맞춰 PowerShell 프로필, `~/.bashrc`, `~/.zshrc`에 훅을 추가합니다. `--shell powershell|bash|zsh|all`로 고를 수 있고, Windows의 Git Bash는 bash로 칩니다. `cdg term uninstall`로 훅을 제거합니다.

명령은 원래 세션에서 그대로 실행되므로 `cd`, 변수, 함수가 모두 평소처럼 동작하고, 히스토리에도 입력한 그대로 남습니다. 편집기, 페이저, REPL, `ssh`는 기록하지 않습니다.

```bash
cdg term install                 # 찾은 셸 전부
cdg term install --shell bash    # 셸 하나만
cdg term uninstall
```

바뀌는 것은 각 시작 파일에 들어가는 표시된 블록 하나이며, `# >>> codigest terminal capture >>>`와 `# <<< codigest terminal capture <<<` 사이에 있습니다. 새로 연 셸 창부터 적용됩니다. 지금 창에서 켜는 줄은 설치할 때 출력해 줍니다.

### 조절하기

- 명령 하나만 기록하지 않으려면 맨 앞에 공백을 넣습니다.
- 기록하지 않을 프로그램을 더 지정하려면: PowerShell에서는 `$env:CODIGEST_CAP_SKIP = '^\s*(myrepl|mytui)\b'`, bash/zsh에서는 `export CODIGEST_CAP_SKIP='^(myrepl|mytui)( |$)'`(확장 정규식).
- 지금 세션에서만 끄려면: PowerShell에서는 `$env:CODIGEST_NOCAP = 1`, bash/zsh에서는 `export CODIGEST_NOCAP=1`.
- 훅을 없애려면: `cdg term uninstall`.

현재 셸에서 훅이 켜져 있지 않으면 `cdg term`이 알려 주고, 그 창에서 켜는 방법도 보여 줍니다.

### 훅이 동작하는 방식

**PowerShell.** 훅이 `PSConsoleHostReadLine`을 대신합니다. PSReadLine이 내가 입력한 줄을 돌려준 뒤, 훅은 그 줄을 호출한 쪽의 스코프에서 실행하며 출력을 복제하는 형태로 감싸서 엔진에 넘깁니다. 화면과 히스토리에는 입력한 그대로 남습니다.

**bash와 zsh.** Enter를 누르면 줄을 기록해 둡니다. 명령이 실행되기 직전에 셸의 stdout과 stderr를 `tee`를 거쳐 비공개 임시 파일로 보내고, 다음 프롬프트 전에 원래대로 돌려놓습니다.

두 경우 모두 완성된 기록은 백그라운드 프로세스에서 `cdg term ingest`로 넘어가며, 비밀정보 가리기는 디스크에 쓰기 전에 거기서 이루어집니다. ingest가 실패하면 기록 옆의 `ingest-errors.log`에 남습니다.

### 알아 둘 점

- 기록되는 프로그램은 터미널이 아니라 파이프로 출력하게 되므로, 색이나 진행 막대가 사라지는 프로그램이 있습니다.
- bash(4 이상)와 zsh에서는 여러 줄에 걸친 명령(여러 줄로 입력한 반복문, heredoc), 백그라운드 작업(`cmd &`), `!` 히스토리 확장을 쓴 줄은 기록하지 않고 그냥 실행합니다. bash에서는 서브셸 `( ... )`로 시작하는 줄도 마찬가지입니다.
- bash 훅은 DEBUG 트랩을 씁니다. 첫 프롬프트 전에 설정된 트랩(starship, bash-preexec)은 그대로 두지만, 그 뒤에 새로 설정한 DEBUG 트랩은 훅을 덮어씁니다.

## 저장되는 내용

기록 하나에는 명령, 출력, 종료 코드, 작업 폴더, 셸, 호스트 이름, 시작 시각, 걸린 시간이 들어 있습니다.

- **정리.** 색 코드는 지우고, 진행 막대처럼 같은 줄을 덮어쓰는 출력(캐리지 리턴)은 마지막 상태만 남깁니다. 그래서 `pytest`나 `pip`을 한 번 실행해도 수 MB의 화면 갱신이 쌓이지 않습니다.
- **가리기.** 토큰과 `password=...` 형태의 값은 기록하기 전에 가립니다. 인증 정보를 출력하는 명령은 출력을 아예 저장하지 않습니다. [보안](/ko/security#in-terminal-captures)을 참고하세요.
- **상한.** 한 줄에 하나씩 JSON으로 적는 파일 하나에 최근 명령 200개를 보관합니다. 출력은 명령 하나당 64 KB까지 저장하고, 넘치면 앞 32 KB와 뒤 32 KB만 남기고 가운데에 표시를 넣습니다. 이런 기록에는 블록에 `truncated="true"`가 붙습니다.
- **위치.** `cdg term path`로 파일을 확인할 수 있습니다. macOS와 Linux에서는 본인만 읽을 수 있게 저장합니다. `CODIGEST_HOME`으로 옮길 수 있습니다. [구조](/ko/architecture#where-things-are-stored)를 참고하세요.
