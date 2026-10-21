# 설치

[uv](https://docs.astral.sh/uv/)로 설치합니다. uv가 필요한 Python(3.15)까지 알아서 받아 두므로, 내 프로젝트의 Python 환경과 섞이지 않습니다.

```bash
winget install --id astral-sh.uv                  # Windows (uv가 이미 있으면 건너뛰기)
curl -LsSf https://astral.sh/uv/install.sh | sh   # macOS/Linux

uv tool install codigest
uv tool update-shell                              # 끝나면 터미널을 다시 열기
```

명령 이름은 `codigest`이고, 줄여서 `cdg`로도 쓸 수 있습니다. 프로젝트마다 따로 설정할 것은 없습니다.

잘 설치되었는지 확인합니다.

```bash
cdg --version
cdg --help
```

::: tip 설치할 때 `lazy import` 부분에서 문법 오류가 난다면
Codigest는 Python 3.15 문법을 쓰는데, 오래된 uv는 그 Python을 받지 못할 수 있습니다. `uv self update`를 한 뒤 다시 설치하세요.
:::

## PowerShell 도움말

PowerShell의 `help codigest`는 `codigest --help`와 달리 PowerShell의 `Get-Help`를 호출합니다. 터미널 자동 기록 기능을 켜지 않고도 다음처럼 별도로 연결할 수 있습니다.

```powershell
cdg shell-help install       # PowerShell 프로필에 도움말 등록
# PowerShell 새 창 열기
help codigest                # 또는 Get-Help codigest
cdg shell-help uninstall     # 등록 취소
```

다른 PowerShell 명령의 `help` 동작은 바꾸지 않고, Codigest CLI를 연결하는 함수만 프로필에 추가합니다. CLI의 전체 명령 설명은 `codigest --help`로도 볼 수 있습니다.

## 선택: 모든 명령 기록하기

`cdg term install`로 셸에 훅을 설치하면 모든 명령의 출력이 `cdg pack`과 `cdg term`을 위해 보관됩니다. 직접 실행하기 전에는 꺼져 있습니다. [터미널 기록](/ko/terminal-capture)을 참고하세요.

## 내려받은 소스에서 실행하기

Codigest 자체를 고쳐 보면서 설치 없이 시험하려면 이렇게 합니다.

```bash
git clone https://github.com/NaNIsaac/codigest
cd codigest
uv run cdg --help
```

## 제거

셸 훅을 설치했다면 먼저 제거하고, 그다음 도구를 지웁니다.

```bash
cdg term uninstall
cdg shell-help uninstall     # PowerShell에서 썼다면
uv tool uninstall codigest
```

기록된 명령은 프로젝트 밖에 있으므로 `cdg term clear`로 지웁니다. 프로젝트의 `.codigest/` 폴더는 `cdg clean`으로 지웁니다. 위치는 [구조](/ko/architecture#where-things-are-stored)에 있습니다.
