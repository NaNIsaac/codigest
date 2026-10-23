# 설치

## 빌드된 바이너리 (권장)

네이티브 바이너리 하나이고 Python이 필요 없습니다. Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.ps1 | iex
```

Linux, macOS, Termux:

```bash
curl -fsSL https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.sh | sh
```

스크립트가 내 시스템에 맞는 최신 릴리스를 받아 `cdg`를 설치합니다. Windows는 `%LOCALAPPDATA%\Programs\codigest`에 설치하고 사용자 `PATH`에 추가하며, Linux와 macOS는 `~/.local/bin`, Termux는 `$PREFIX/bin`에 설치합니다. 끝나면 터미널을 새로 여세요. 같은 줄을 다시 실행하면 업데이트됩니다.

지원하는 환경은 Windows x64, Linux x64와 ARM64(정적 링크, musl), macOS Intel과 Apple Silicon, Android ARM64(Termux)입니다.

환경 변수 두 개로 동작을 바꿀 수 있습니다. `CDG_VERSION=0.12.1`은 최신 버전 대신 그 버전을 설치합니다. `CDG_INSTALL_DIR=...`은 그 폴더에 설치하며, 이때는 `PATH`를 건드리지 않습니다.

```powershell
$env:CDG_VERSION = "0.12.1"; irm https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.ps1 | iex
```

```bash
curl -fsSL https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.sh | CDG_INSTALL_DIR=/opt/codigest sh
```

압축 파일을 직접 받으려면 [Rust 포팅](/ko/rust-port#install)을 참고하세요. 그 페이지에 속도 수치와 Python 패키지와의 차이도 있습니다.

명령 이름은 `cdg`입니다. 프로젝트마다 따로 설정할 것은 없습니다.

잘 설치되었는지 확인합니다.

```bash
cdg --version
cdg --help
```

## Python 패키지

같은 명령을 PyPI에 `codigest`라는 이름으로도 올려 두었습니다. [uv](https://docs.astral.sh/uv/)로 설치하면 필요한 Python(3.15)까지 알아서 받아 두므로, 내 프로젝트의 Python 환경과 섞이지 않습니다. 그 Python이 이미 있다면 pip와 pipx도 됩니다.

```bash
winget install --id astral-sh.uv                  # Windows (uv가 이미 있으면 건너뛰기)
curl -LsSf https://astral.sh/uv/install.sh | sh   # macOS/Linux

uv tool install codigest
uv tool update-shell                              # 끝나면 터미널을 다시 열기
```

명령 이름은 `codigest`이고, 줄여서 `cdg`로도 쓸 수 있습니다.

::: warning `PATH`에는 `cdg`를 하나만 두세요
바이너리와 Python 패키지는 둘 다 `cdg`라는 명령을 설치하고, `PATH`에서 먼저 나오는 쪽이 실행됩니다. 둘 중 하나만 쓰세요. MCP 레지스트리 항목은 Python 패키지를 실행하며(`uvx codigest mcp`), 이 방식은 `PATH`에 아무것도 추가하지 않습니다.
:::

::: tip 설치할 때 `lazy import` 부분에서 문법 오류가 난다면
Python 패키지는 Python 3.15 문법을 쓰는데, 오래된 uv는 그 Python을 받지 못할 수 있습니다. `uv self update`를 한 뒤 다시 설치하세요.
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

## 다음 단계: 모든 명령 기록하기

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
```

그다음 바이너리를 지웁니다. Windows는 `%LOCALAPPDATA%\Programs\codigest` 폴더(와 사용자 `PATH`의 해당 항목), 그 밖에는 `~/.local/bin/cdg`(Termux는 `$PREFIX/bin/cdg`)입니다. Python 패키지라면 대신 `uv tool uninstall codigest`를 실행합니다.

기록된 명령은 프로젝트 밖에 있으므로 `cdg term clear`로 지웁니다. 프로젝트의 `.codigest/` 폴더는 `cdg clean`으로 지웁니다. 위치는 [구조](/ko/architecture#where-things-are-stored)에 있습니다.
