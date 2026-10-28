# Rust 포팅

::: info 상태: 권장 설치 방법
Python 빌드의 모든 명령을 Rust로 옮겼고, 빌드된 바이너리가 Codigest의 권장 설치 방법입니다([설치](/ko/installation) 참고). PyPI의 Python 패키지도 그대로 쓸 수 있고, 이 사이트의 나머지 페이지는 둘 다에 해당합니다.
:::

포팅한 결과는 네이티브 바이너리 하나이며, 저장소의 Python 패키지 옆(`rust/`)에서 빌드합니다. 기준은 Python 빌드(`src/codigest`)입니다. 같은 입력에 대해 Rust 빌드는 같은 바이트를 stdout으로 출력하고 같은 종료 코드로 끝나야 합니다.

두 빌드 모두 `cdg`라는 명령을 설치하므로 `PATH`에는 하나만 두세요. 두 프로그램은 별개이고, 같은 데이터를 읽고 씁니다. 터미널 기록 저장소, 프로젝트의 `.codigest/`(설정, 기준점, 캐시), 셸 훅이 그것입니다.

## 설치 {#install}

Windows(PowerShell)에서는 설치 스크립트가 `cdg.exe`를 `%LOCALAPPDATA%\Programs\codigest`에 넣고 그 폴더를 사용자 `PATH`에 추가합니다.

```powershell
irm https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.ps1 | iex
```

Linux, macOS, Termux에서는 `cdg`를 `~/.local/bin`(Termux는 `$PREFIX/bin`)에 설치합니다.

```bash
curl -fsSL https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.sh | sh
```

같은 줄을 다시 실행하면 업데이트됩니다. `CDG_VERSION=0.12.1`로 버전을 고정하고, `CDG_INSTALL_DIR=...`로 폴더를 정하며, 이때는 `PATH`를 건드리지 않습니다.

바이너리는 `rust-v*` 태그의 GitHub 릴리스에 있습니다. Windows x64, Linux x64와 ARM64(정적 링크, musl), macOS Intel과 Apple Silicon, Termux용 Android ARM64입니다. 압축 파일 이름은 `codigest-X.Y.Z-<target>.zip`(Windows) 또는 `.tar.gz`이고, 안에는 `codigest-X.Y.Z-<target>/` 폴더가 있으며 그 안에 `cdg`(Windows는 `cdg.exe`)와 라이선스가 들어 있습니다.

직접 설치하려면 압축 파일을 내려받습니다.

Windows (PowerShell):

```powershell
$v = "0.13.0"; $t = "x86_64-pc-windows-msvc"
Invoke-WebRequest "https://github.com/NaNIsaac/codigest/releases/download/rust-v$v/codigest-$v-$t.zip" -OutFile codigest.zip
Expand-Archive codigest.zip "$env:LOCALAPPDATA\Programs" -Force
# 그다음 $env:LOCALAPPDATA\Programs\codigest-$v-$t 를 PATH에 추가
```

Linux, macOS, Termux (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `aarch64-linux-android` 중 하나):

```bash
v=0.13.0; t=x86_64-unknown-linux-musl
curl -L "https://github.com/NaNIsaac/codigest/releases/download/rust-v$v/codigest-$v-$t.tar.gz" | tar xz
install "codigest-$v-$t/cdg" ~/.local/bin/
```

GitHub CLI로는 `gh release download rust-v0.13.0 -R NaNIsaac/codigest -p '*windows*'`.

## 빌드

Rust 툴체인이 필요합니다. 저장소를 내려받은 폴더에서 다음을 실행합니다.

```bash
cd rust
cargo build --release        # target/release/cdg
cargo test                   # 단위 테스트와 통합 테스트
CODIGEST_PARITY=1 cargo test # PATH의 Python `cdg`와도 비교
```

릴리스 바이너리는 4.6 MB이고 Python이 필요 없습니다.

## 포팅된 범위

모든 명령을 같은 옵션으로 옮겼습니다: `scan`, `digest`, `tree`, `mark`, `diff`, `semdiff`, `cap`, `pack`, `term`(`clear`, `path`, `install`, `uninstall`, 훅이 호출하는 `ingest` 포함), `mcp`, `status`, `init`, `clean`, `shell-help`.

이 빌드에만 있는 것: `term --raw`는 명령 하나의 저장된 출력을 태그 없이 그대로 출력합니다. [명령어 레퍼런스의 안내](/ko/commands#term-raw)를 참고하세요. 이 빌드에만 있는 다른 기능은 아래에 정리했습니다. `scan`, `term`의 `--format markdown|json`과 `--preset NAME`은 두 빌드가 같이 지원하며, [명령어 레퍼런스](/ko/commands#formats)에 있습니다.

## 이 빌드에만 있는 기능

### `digest`가 읽는 언어

Python, MATLAB 말고도 Rust, JavaScript(`.js`, `.jsx`, `.mjs`, `.cjs`), TypeScript(`.ts`, `.tsx`), Go의 개요를 만듭니다. 정의마다 번호 붙은 한 줄, 타입이나 `impl` 아래에 멤버를 들여쓰고, 문서 주석의 첫 줄을 그 밑에 둡니다. Rust의 `#[cfg(test)]` 아래 항목은 뺍니다. 이 저장소에서 `cdg digest rust/src/presets.rs`를 실행한 결과의 일부입니다.

```text
 1: //! `--preset NAME`: option sets kept in TOML, expanded into the command line before it is
25: pub fn user_config() -> PathBuf
        `%APPDATA%\codigest\config.toml` on Windows, else `$XDG_CONFIG_HOME/codigest/config.toml`
40: fn project_config(start: &Path) -> Option<PathBuf>
        The nearest `.codigest/config.toml` from `start` upwards, stopping at a Git root.
93: pub fn expand(argv: Vec<OsString>) -> Result<Vec<OsString>>
```

## 같은 결과인지 확인하는 방법

- 통합 테스트가 임시 프로젝트에서 두 빌드로 명령을 각각 실행해 stdout을 바이트 단위로, 종료 코드를 함께 비교합니다. `CODIGEST_PARITY=1 cargo test`로 켜며, `target/.../cdg`를 `PATH`의 Python `cdg`와 비교하므로 셸이 찾는 `cdg`는 Python 패키지여야 합니다.
- `digest`가 쓰는 Python 개요는 `ast.unparse`와 같은 결과를 냅니다. `scripts/outline_parity.py`가 두 빌드를 파일별로 비교했고, 프로젝트 옆에 내려받아 둔 저장소 40개의 파일 1,747개에서 차이가 없었습니다.
- import 해석(`-r`, `pack`)은 저장소 다섯 곳에서 비교했고 차이가 없었습니다.

## 속도

[hyperfine](https://github.com/sharkdp/hyperfine)으로 잰 실행 시간입니다(`-N`, 워밍업 2회, 15회 측정, 파일 10,000개 항목은 5회). 평균 ± 표준편차이고 단위는 밀리초입니다. AMD Ryzen 5 7500F(6코어, 12스레드), Windows 11, Python 3.15.0에서 쟀고, Python 빌드는 같은 커밋을 `uv tool install --editable`로 설치한 것입니다. 출력은 파이프로 보냈으므로 클립보드에는 복사하지 않았습니다. 파일 1,000개와 10,000개짜리 프로젝트는 생성한 Python 패키지입니다(10,000개일 때 6.4 MB).

| 명령 | Python | Rust | 배율 |
| --- | ---: | ---: | ---: |
| `--version` | 245 ± 10 | 6.2 ± 1.1 | 39.8x |
| `term -n 5` | 257 ± 7 | 7.5 ± 0.4 | 34.4x |
| `tree`, 이 저장소(파일 198개) | 277 ± 6 | 11.9 ± 0.7 | 23.2x |
| `scan -y`, 이 저장소 | 510 ± 8 | 27.0 ± 1.0 | 18.9x |
| `digest`, xarray(파일 503개) | 3,142 ± 45 | 255.9 ± 9.0 | 12.3x |
| `tree`, 파일 1,000개 | 453 ± 11 | 52.0 ± 1.0 | 8.7x |
| `scan -y`, 파일 1,000개 | 661 ± 11 | 108.0 ± 3.0 | 6.1x |
| `digest`, 파일 1,000개 | 1,074 ± 17 | 107.8 ± 5.6 | 10.0x |
| `tree`, 파일 10,000개 | 1,818 ± 14 | 319.9 ± 5.4 | 5.7x |
| `scan -y`, 파일 10,000개 | 3,705 ± 38 | 889.4 ± 9.6 | 4.2x |
| `digest`, 파일 10,000개 | 7,811 ± 36 | 838.1 ± 17.3 | 9.3x |

## Python 빌드와 다른 점 {#differences-from-the-python-build}

- `term --raw`는 새로 생긴 옵션입니다. 둘 이상의 기록을 고르면 출력 사이의 경계가 사라지므로, 이어 붙이지 않고 종료 코드 2로 멈춥니다.
- 구문이 다른 곳에서 깨진 Python 파일도, 파싱되는 부분의 개요는 보여 줍니다. Python 빌드는 이런 파일에 아무것도 보여 주지 않습니다. (코드에 해석할 수 없는 바이트가 있으면 두 빌드 모두 개요가 없습니다.)
- 실패한 테스트가 여러 모듈의 함수를 호출하면, `pack`은 그 모듈들을 테스트 안에서 호출이 나타나는 순서로 늘어놓습니다. Python 빌드는 `ast.walk`의 너비 우선 순서를 따릅니다.
- 터미널에 직접 출력하는 안내 줄(`init`, `clean`, `term clear`)은 Windows에서 `\n`으로 끝납니다. Python은 `\r\n`으로 씁니다. 명령 결과 자체(`scan`, `pack` 등의 stdout과 `-o` 파일)는 Python처럼 플랫폼의 줄 끝을 씁니다.
- `cap`은 Ctrl+C를 명령에 전달하고, 강제 종료하기 전에 5초를 기다립니다.

이 사이트의 나머지 페이지는 두 빌드에 모두 해당합니다.
