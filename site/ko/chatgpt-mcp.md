# ChatGPT 비공개 연결

Codigest는 프로젝트가 있는 컴퓨터에서 돕니다. 다른 ChatGPT 실행 환경에 설치해도 내 컴퓨터에 접근할 수는 없습니다. 비공개 사용자 지정 MCP 연결에는 공개 디렉터리 등록이 필요 없습니다.

## 1. 설치하고 시험하기

`scan_github`가 들어 있는 Python 릴리스를 쓰거나, 검토한 소스 브랜치를 로컬에 설치합니다.

```powershell
uv tool install --python 3.15 --editable .
cd PATH_TO_A_SMALL_TEST_PROJECT
codigest mcp --allow-remote
```

이 명령은 줄 단위 MCP 요청을 기다립니다. 시작되는 것을 확인했으면 Ctrl+C로 멈춥니다. 시험용 프로젝트는 작게 둡니다. 이 서버는 로컬 도구도 열어 두며, 원격 모드라고 로컬 접근이 꺼지지는 않습니다. 터널이나 예약 작업으로 돌릴 때도 같은 계정, 같은 작업 폴더, 같은 실행 파일 절대 경로를 씁니다.

## 2. 비공개 터널 만들기

https://developers.openai.com/api/docs/guides/secure-mcp-tunnels 를 따릅니다. https://platform.openai.com/settings/organization/tunnels 에서 터널을 만들고 쓸 ChatGPT 작업 공간을 연결합니다. 만들려면 Tunnels Read + Manage, 쓰려면 Read + Use 권한이 필요합니다. 사용자 지정 MCP를 쓸 수 있는지와 터널 권한은 따로입니다. 계정에서 쓸 수 있는지와 요금을 확인합니다. 이 안내는 무료를 약속하지 않습니다.

같은 설정 페이지에서 `tunnel-client`를 내려받습니다. `CONTROL_PLANE_API_KEY`는 안전한 환경 변수나 비밀값 관리 도구로 로컬에만 설정합니다. 그 값을 채팅, 명령 인자, 이 저장소, 저장된 프로필에 넣지 않습니다. 그다음 시험용 프로젝트에서 실행합니다.

```powershell
tunnel-client help quickstart
tunnel-client init --sample sample_mcp_stdio_local --profile codigest --tunnel-id YOUR_TUNNEL_ID --mcp-command "codigest mcp --allow-remote"
tunnel-client doctor --profile codigest --explain
tunnel-client run --profile codigest
```

명령을 찾지 못하면 `Get-Command codigest`가 알려 주는 실행 파일 절대 경로를 씁니다. run 프로세스는 계속 살아 있어야 합니다. 파일을 읽기 전에 `tree` 도구로 작업 폴더를 확인합니다. 다시 시작한 뒤에도 프로필이 셸의 폴더를 그대로 둔다고 가정하지 않습니다.

## 3. 연결하고 확인하기

https://chatgpt.com/plugins 에서 **+ → Add custom MCP server → Connection: Tunnel**을 고르고, 같은 터널을 골라 비공개 플러그인을 만들고 설치합니다. 이 항목이 없으면 인증 없는 공개 엔드포인트로 바꾸지 말고 멈춘 뒤 계정과 작업 공간 권한을 확인합니다.

새 채팅을 열고 플러그인을 고른 뒤 확인합니다.

1. `tree`가 홈 폴더나 저장소 루트가 아니라 의도한 시험용 프로젝트를 돌려줍니다.
2. `scan`이 직접 만든 무해한 시험 파일을 돌려줍니다.
3. `scan_github`가 작은 공개 저장소나 경로를 읽고 확정된 커밋을 알려 줍니다.
4. 운영자가 `--allow-remote --auth`로 다시 시작하지 않았다면 비공개 접근은 되지 않습니다.

ChatGPT까지 이어진 접근은 도구 호출이 실제로 성공하는 것을 봐야 확인됩니다. 로컬 시험과 서버 시작만으로는 확인되지 않습니다. 도구나 스키마를 바꾸면 플러그인 연결을 새로 고칩니다. 터널은 연결만 줍니다. 백그라운드 자율 실행이나 Tailscale 접근은 주지 않습니다.
