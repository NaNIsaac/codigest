# 보안

결과는 내가 붙여 넣은 곳, 또는 내가 연결한 MCP 클라이언트로만 갑니다. Codigest 자체는 네트워크를 쓰지 않고 AI 모델도 호출하지 않습니다.

아래는 그 결과에 비밀정보가 섞이지 않게 하려고 하는 일입니다. 이 규칙들은 흔한 형태를 잡아내는 것이고, 모든 비밀정보를 잡을 수는 없습니다. 붙여 넣기 전에 내용을 한 번 읽어 보세요.

## 절대 넣지 않는 것 {#what-is-never-included}

인증 정보 파일은 어떤 명령도 넣지 않습니다. `scan`, `digest`, `tree`, `diff`, `semdiff`, `pack`, MCP 도구 모두 건너뜁니다.

| 패턴 | 예 |
| --- | --- |
| `.env`와 `.env.*` | `.env`, `.env.local`, `.env.production` |
| 키·인증서 파일 | `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.keystore`, `*.jks` |
| SSH 개인 키 | `id_rsa`, `id_dsa`, `id_ecdsa`, `id_ed25519` |
| 인증 정보 저장소 | `.netrc`, `.pgpass` |

값 없이 변수만 설명하는 템플릿은 넣습니다: `.env.example`, `.env.sample`, `.env.template`, `.env.dist`.

명령은 무엇을 뺐는지 알려 줍니다. `scan`은 파일 이름을 말해 주고, `pack`은 내가 지정했거나 고친 파일이면 `<omitted>`에 올리며, diff에서는 해당 부분을 안내 문구로 바꿉니다. 코드 옆에 `.env`와 `deploy.pem`이 있을 때의 결과입니다.

```console
$ cdg scan
Withheld (may contain secrets): .env, deploy.pem
<project_snapshot project="secrets">
<project_structure>
├── .env.example
└── app.py
</project_structure>
<source_code>
<file path=".env.example">
API_KEY=placeholder
</file>

<file path="app.py">
import requests

OPENAI_API_KEY = "[REDACTED]"
token = get_token()
</file>
</source_code>
</project_snapshot>
```

`.git/`, `.codigest/`, `.venv/`, `venv/`, `env/`, `__pycache__/`, `node_modules/`, `.idea/`, `.vscode/`, `*.pyc`, `*.DS_Store`와, `.gitignore`나 [설정](/ko/configuration)에서 뺀 것도 스캔하지 않습니다.

## 가리는 것

가리는 단계가 두 겹이고, 적용 시점이 다릅니다.

### 출력할 때

명령이 출력하거나 복사하거나 파일로 쓰는 모든 내용은 나가기 직전에 같은 필터를 한 번 거칩니다. 누가 봐도 비밀인 것만 가리므로, 평범한 코드는 그대로 둡니다.

| 대상 | 바뀌는 모습 |
| --- | --- |
| 개인 키 블록(`-----BEGIN ... PRIVATE KEY-----`부터 `END`까지) | `-----BEGIN PRIVATE KEY-----[REDACTED]-----END PRIVATE KEY-----` |
| JWT | `[REDACTED]` |
| OpenAI 키(`sk-...`) | `[REDACTED]` |
| GitHub 토큰(`ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`, `github_pat_`) | `[REDACTED]` |
| AWS 액세스 키 ID(`AKIA...`, `ASIA...`) | `[REDACTED]` |
| Google API 키(`AIza...`) | `[REDACTED]` |
| Slack 토큰(`xox...`) | `[REDACTED]` |
| GitLab 토큰(`glpat-`) | `[REDACTED]` |
| npm 토큰(`npm_`) | `[REDACTED]` |
| URL 속 비밀번호(`scheme://user:password@host`) | 비밀번호가 `[REDACTED]`로 바뀜 |

소스에 있는 `token = get_token()`이나 `Authorization: Bearer {token}`은 건드리지 않습니다. `OPENAI_API_KEY = "sk-..."` 같은 줄은 값이 알아볼 수 있는 모양이라서 가립니다.

### 터미널 기록에서 {#in-terminal-captures}

터미널 출력에는 소스 코드보다 인증 정보가 섞이기 쉬우므로, 기록은 더 엄격하게 다룹니다. 읽을 때가 아니라 **디스크에 쓰기 전에** 처리합니다.

- 위 표의 모든 것.
- `Authorization: Bearer|Basic|Token ...` 헤더: 인증 값을 가립니다.
- 키가 `api_key`, `apikey`, `secret`, `access_token`, `auth_token`, `token`, `password`, `passwd`, `client_secret`, `private_key`(단어 사이에 `-`, `_`가 있거나 없어도 됨)이고 값이 여섯 글자 이상인 `key=value`, `key: value` 형태.
- 명령줄 자체도 같은 처리를 합니다.

```console
$ cdg cap -- echo password=hunter2hunter2
password=hunter2hunter2

$ cdg term
<terminal_history>
<command index="1" command="echo password=[REDACTED]" exit_code="0" cwd="~/secrets" duration="27ms" at="2026-10-21T23:31:24+09:00">
<output>
password=[REDACTED]
</output>
</command>
</terminal_history>
```

화면에서 본 내용은 그대로이고, 저장되는 사본과 Codigest가 내보내는 결과만 가려집니다. bash와 zsh 훅에서는 `cdg term ingest`가 가려서 저장하기 전까지 원본 출력이 비공개 임시 파일에 있습니다.

## 거부하는 명령

어떤 명령은 정의상 인증 정보를 출력합니다. 이런 명령의 출력은 내용과 상관없이 저장하지 않고, 기록에는 안내 문구만 남깁니다.

| 명령 | |
| --- | --- |
| `env`, `printenv` | 인자 없는 `set`, `export` |
| `Get-ChildItem env:`(`gci`, `ls`, `dir`도 같음) | `$env:...`를 `echo` / `Write-Output` |
| `.env` 파일을 `cat`, `type`, `bat`, `Get-Content` | `aws configure` |
| `gh auth token` | `docker login` |
| `security find-generic-password` | `kubectl get secret...` |

`cap`이 실행하는 동안 터미널에는 출력이 그대로 보이고, 저장된 기록만 비어 있습니다. `cdg cap -- env`와 위의 `echo`를 실행한 뒤의 모습입니다.

```console
$ cdg term -n 2
<terminal_history>
<command index="2" command="env" exit_code="0" cwd="~/secrets" duration="13ms" at="2026-10-21T23:31:24+09:00">
<note>output withheld (command may expose credentials)</note>
<output>

</output>
</command>
<command index="1" command="echo password=[REDACTED]" exit_code="0" cwd="~/secrets" duration="27ms" at="2026-10-21T23:31:24+09:00">
<output>
password=[REDACTED]
</output>
</command>
</terminal_history>
```

명령줄의 맨 앞을 대소문자 구분 없이 비교합니다. 다른 명령으로 시작하는 파이프라인(`cat x | env`)은 걸리지 않으니, 공유하기 전에 기록을 한 번 살펴보는 편이 좋습니다.

## 한계

- 규칙은 흔한 형태만 압니다. 무작위 문자열이나 알아볼 만한 접두어가 없는 직접 만든 토큰처럼 모르는 형태의 비밀정보는 가리지 못합니다. `[REDACTED]`는 안전망일 뿐 보장이 아닙니다.
- 위 목록에 없는 이름의 파일에 든 비밀정보(예: `config.json` 속 키)는 파일 이름으로는 알아내지 못하고, 알아볼 수 있는 모양만 가립니다.
- 가리기는 기록하는 시점에 이루어집니다. 업그레이드 전에 만든 기록은 그 버전의 규칙으로 가려져 있습니다.
- 기록이 없을 때 쓰는 셸 히스토리에는 출력이 없습니다. 거기서 가져온 명령은 MCP로 돌려주기 전에 똑같이 가립니다.

## 내 프로젝트와 컴퓨터

- **어떤 명령도 소스 파일을 수정하지 않습니다.** `.codigest/` 폴더는 `mark`와 `init`만 만들며, 폴더 안의 `.gitignore`(`*`) 덕분에 `git status`에 나타나지 않습니다. `init`은 프로젝트의 `.gitignore`에도 이 폴더를 추가합니다.
- **기록은 내 컴퓨터에만 있습니다.** `cdg term path`로 파일을 확인할 수 있습니다. macOS와 Linux에서는 본인만 읽을 수 있게 저장하고, `cdg term clear`로 지울 수 있습니다.
- **크기 제한.** 기록 저장소는 최근 명령 200개를 보관하고, 출력은 하나당 64 KB로 자르므로 출력이 유난히 많은 명령 하나가 저장소를 채우지 못합니다.
- **에이전트.** MCP 도구도 가린 텍스트를 돌려주며 프로젝트 밖의 경로는 거부합니다. [MCP 연동](/ko/mcp#limits)을 참고하세요.
