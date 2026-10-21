# 설정

Codigest는 설정 없이도 동작합니다. 프로젝트의 `.codigest/` 폴더에 아래 파일을 두면 동작을 바꿀 수 있습니다.

| 파일 | 용도 |
| --- | --- |
| `.codigest/config.toml` | 모든 명령이 다룰 파일을 정합니다. |
| `.codigest/prompts.toml` | `scan`, `diff`, `semdiff`, `digest`의 출력에 쓸 내 템플릿. |

`cdg init`이 시작용 `config.toml`을 만들어 줍니다. `prompts.toml`은 직접 만듭니다.

프로젝트 루트는 현재 폴더, 또는 그 위로 올라가며 처음 만나는 `.codigest/` 또는 `.git/`이 있는 폴더입니다. `.codigest/`는 거기서 찾습니다.

## config.toml

```toml
[filter]
# 이 확장자만 다룹니다 (지우면 무시 목록에 없는 모든 파일)
extensions = [".py", ".ts", ".rs", ".md", ".json"]

# 추가로 뺄 파일, .gitignore 문법 (.gitignore는 항상 따릅니다)
exclude_patterns = [
    "*.lock",
    "tests/data/",
    "legacy_code/"
]
```

### 적용되는 키

현재 읽는 키는 `[filter]` 아래의 두 개입니다.

| 키 | 효과 |
| --- | --- |
| `extensions` | 남길 파일 확장자의 목록. 앞의 점을 포함해 소문자로 씁니다(`".py"`). 다른 확장자의 파일은 건너뜁니다. 다만 `.gitignore`, `Dockerfile`, `pyproject.toml`은 항상 남깁니다. 목록을 지우거나 비워 두면 모든 파일을 남깁니다. |
| `exclude_patterns` | `.gitignore` 문법의 패턴 목록. 프로젝트의 `.gitignore` 규칙에 더해집니다. |

필터는 파일 목록을 만드는 모든 명령에 적용됩니다: `scan`, `digest`, `tree`, `diff`, `semdiff`, `mark`, `status`, `pack`. 명령줄에서 직접 지정한 파일은 `extensions` 때문에 빠지지 않습니다. `scan -a`와 `tree -a`는 그 실행에서만 두 키를 모두 무시합니다.

설정과 상관없이 항상 빼는 것은 `.git/`, `.codigest/`, `.venv/`, `venv/`, `env/`, `__pycache__/`, `node_modules/`, `.idea/`, `.vscode/`, `*.pyc`, `*.DS_Store`입니다. 프로젝트 루트의 `.gitignore`는 항상 따릅니다.

`config.toml`을 읽을 수 없으면 아무 말 없이 무시하고 필터를 적용하지 않습니다. `cdg status`를 실행하면 읽은 확장자를 볼 수 있습니다. 파일이 있으면 `filters` 줄이 나옵니다.

### 적용되지 않는 키

`cdg init`은 `[project]`, `[filter]` 아래의 `max_file_size_kb`, 그리고 `format`과 `structure`가 든 `[output]` 섹션도 함께 써 줍니다. 지금은 이것들을 읽는 곳이 없습니다. 바꿔도 아무 효과가 없으며, 지워도 됩니다.

## prompts.toml {#prompts-toml}

템플릿은 명령 하나의 출력에서 기본 포장을 대신합니다. `[prompts]` 표 아래에 명령마다 키 하나씩 둡니다.

```toml
[prompts]
snapshot = """Review this code for bugs.
{instruction}
Project: {project_name}
{tree_structure}
{source_code}"""
```

글은 Python의 `str.format`으로 채우므로, 자리표시자는 중괄호 안에 쓰고 중괄호 문자 자체는 두 번 씁니다(<code v-pre>{</code>는 <code v-pre>{{</code>). 쓸 수 있는 이름은 명령마다 다릅니다.

| 키 | 명령 | 자리표시자 |
| --- | --- | --- |
| `snapshot` | `scan` | `project_name`, `tree_structure`, `source_code`, `instruction` |
| `diff` | `diff` | `project_name`, `context_message`, `diff_content`, `instruction` |
| `semdiff` | `semdiff` | `project_name`, `context_message`, `semdiff_content`, `instruction` |
| `digest` | `digest` | `project_name`, `tree_structure`, `digest_content`, `instruction` |

`instruction`은 `-m`으로 준 문구이고, 주지 않으면 비어 있습니다. `source_code`와 `digest_content`에는 이미 완성된 `<file>` 블록이 들어 있습니다. 표에 없는 키는 기본값을 그대로 씁니다.

위 템플릿으로 `cdg scan -m "Why does the test fail?"`를 실행하면 이렇게 출력됩니다.

```text
Review this code for bugs.
Why does the test fail?
Project: project
├── .gitignore
├── prices.py
└── test_prices.py
<file path=".gitignore">
...
```

템플릿에 없는 자리표시자를 쓰면 내용 대신 오류 메시지가 출력됩니다. `pack`, `term`, `cap`은 템플릿을 쓰지 않습니다. 기본값과 달리 템플릿은 XML 루트 요소로 감싸지 않으므로, 적은 내용이 전부입니다.

블록 안의 코드는 그대로 전달되고, `instruction`만 XML 이스케이프됩니다.
