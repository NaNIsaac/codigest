# 기여하기

Codigest는 MIT 라이선스이고 [github.com/NaNIsaac/codigest](https://github.com/NaNIsaac/codigest)에 있습니다. 이슈와 풀 리퀘스트를 환영합니다.

## 준비

[uv](https://docs.astral.sh/uv/)가 필요합니다. Python 3.15는 uv가 받아 줍니다.

```bash
git clone https://github.com/NaNIsaac/codigest
cd codigest
uv run cdg --help               # 내려받은 소스에서 바로 실행
uv run --with pytest pytest -q  # 테스트
uvx ruff@0.17.1 check           # 린트. CI와 같은 버전
```

CI는 `main`에 push할 때와 풀 리퀘스트마다 Ubuntu에서 테스트를, Windows에서 PowerShell 점검(`Get-Help` 연결, Windows PowerShell 5.1 포함)을, 그리고 ruff를 실행합니다.

## 어디에 무엇이 있나

모듈 지도는 [구조](/ko/architecture)에 있습니다. 테스트는 `tests/`에 있고 영역마다 파일이 하나입니다(`test_pack.py`, `test_secrets.py`, `test_termstore.py` 등). Codigest를 Codigest 자신에게 써 본 기록은 `docs/dogfood.md`에 있습니다.

## 지침

- `pyproject.toml`이 정한 Python 버전을 기준으로, 그 버전의 문법과 표준 라이브러리를 씁니다. 의존성은 꼭 필요할 때만 더합니다.
- 주석은 "무엇"이 아니라 "왜"를 적습니다. 이름 짓기, 스타일, 언어는 주변 코드를 따릅니다.
- 명령의 출력은 인터페이스의 일부입니다. 바꾸면 그것을 고정하는 테스트와, 이 사이트에서 그 출력을 보여 주는 페이지도 함께 고칩니다.
- 커밋 메시지는 짧은 conventional commit입니다: `fix: ...`, `feat: ...`, `docs: ...`.

## 이 문서 사이트

사이트는 `site/`에 있는 VitePress이고, 영어와 한국어로 되어 있습니다.

```bash
cd site
npm ci
npm run dev       # 로컬 미리보기 (자동 새로고침)
npm run build     # CI가 실행하는 빌드
```

영어 페이지는 `site/`에, 한국어 페이지는 `site/ko/`에 같은 파일 이름으로 있습니다. 페이지를 고칠 때는 두 쪽을 함께 고치세요.

명령어 레퍼런스의 옵션 표는 CLI와 대조해 확인합니다.

```bash
uv run python site/scripts/check_reference.py
```

문서에 있는 옵션이 실제로 없거나, 실제 옵션이 문서에 빠졌으면 두 언어 모두에서 실패합니다. 사이트에 나온 명령 출력은 실제로 실행한 결과입니다. 명령의 출력을 바꿨다면 다시 실행해서 결과를 붙여 넣으세요.

`site/` 아래를 바꾼 채 `main`에 push하면 사이트를 빌드해 GitHub Pages에 배포합니다.
