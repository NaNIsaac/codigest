# Security

Context goes where you paste it, or to the MCP client you connected. Codigest itself makes no network requests and does not call any AI model.

What follows is what it does to keep secrets out of that context. These rules catch common formats, not every possible secret. Read what you paste.

## What is never included

Credential files are not included by any command: `scan`, `digest`, `tree`, `diff`, `semdiff`, `pack` and the MCP tools all skip them.

| Pattern | Examples |
| --- | --- |
| `.env` and `.env.*` | `.env`, `.env.local`, `.env.production` |
| Key and certificate files | `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.keystore`, `*.jks` |
| SSH private keys | `id_rsa`, `id_dsa`, `id_ecdsa`, `id_ed25519` |
| Credential stores | `.netrc`, `.pgpass` |

Templates that document variables without values are kept: `.env.example`, `.env.sample`, `.env.template`, `.env.dist`.

The commands say what they held back. `scan` names the files, `pack` lists them in `<omitted>` when you named or changed them, and a diff replaces their hunks with a notice. With a `.env` and a `deploy.pem` next to some code:

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

The same folders are also never scanned: `.git/`, `.codigest/`, `.venv/`, `venv/`, `env/`, `__pycache__/`, `node_modules/`, `.idea/`, `.vscode/`, plus `*.pyc` and `*.DS_Store`, and anything your `.gitignore` or [config](/configuration) excludes.

## What is masked

Two layers, applied at different times.

### At output

Everything a command prints, copies or writes passes through one filter just before it leaves. It masks only unambiguous secrets, so ordinary code is left alone:

| What | Replaced with |
| --- | --- |
| Private key blocks (`-----BEGIN ... PRIVATE KEY-----` to `END`) | `-----BEGIN PRIVATE KEY-----[REDACTED]-----END PRIVATE KEY-----` |
| JWTs | `[REDACTED]` |
| OpenAI keys (`sk-...`) | `[REDACTED]` |
| GitHub tokens (`ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`, `github_pat_`) | `[REDACTED]` |
| AWS access key IDs (`AKIA...`, `ASIA...`) | `[REDACTED]` |
| Google API keys (`AIza...`) | `[REDACTED]` |
| Slack tokens (`xox...`) | `[REDACTED]` |
| GitLab tokens (`glpat-`) | `[REDACTED]` |
| npm tokens (`npm_`) | `[REDACTED]` |
| Passwords in URLs (`scheme://user:password@host`) | the password becomes `[REDACTED]` |

`token = get_token()` and `Authorization: Bearer {token}` in source are not touched. A line such as `OPENAI_API_KEY = "sk-..."` is masked because the value has a recognised shape.

### In terminal captures

Terminal output is more likely to hold credentials than source code, so captures get stricter treatment, **before they are written to disk**, not when they are read back:

- Everything in the table above.
- `Authorization: Bearer|Basic|Token ...` headers: the credential is masked.
- `key=value` and `key: value` forms where the key is `api_key`, `apikey`, `secret`, `access_token`, `auth_token`, `token`, `password`, `passwd`, `client_secret` or `private_key` (`-`, `_` or nothing between the words) and the value is six or more characters.
- The command line itself gets the same treatment.

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

What you saw on screen is unchanged; only the stored copy and what Codigest outputs are masked. With the bash and zsh hooks, the raw output sits in a private temporary file until `cdg term ingest` has masked it and stored it.

## Denied commands

Some commands print credentials by definition. The output of these is never stored, whatever it contains, and the record carries a note instead. The terminal still shows the output while `cap` runs it; only the stored record is empty. After `cdg cap -- env` and the `echo` above:

| Command | |
| --- | --- |
| `env`, `printenv` | `set` and `export` with no arguments |
| `Get-ChildItem env:` (also `gci`, `ls`, `dir`) | `echo` / `Write-Output` of `$env:...` |
| `cat`, `type`, `bat`, `Get-Content` of a `.env` file | `aws configure` |
| `gh auth token` | `docker login` |
| `security find-generic-password` | `kubectl get secret...` |

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

Matching is on the start of the command line, case-insensitively. A pipeline that starts with another command (`cat x | env`) is not matched, which is one more reason to review captures before sharing.

## Limits

- The rules know common formats. A secret in a format they do not know, such as a bare random string or a custom token without a recognisable prefix, is not masked. Treat `[REDACTED]` as a safety net, not a guarantee.
- A secret in a file whose name is not on the list above (a key in `config.json`, say) is not detected by name; only its recognisable shapes are masked.
- Masking runs at capture time. Captures made before you upgraded were masked by the rules of that version.
- Native shell history, used when nothing was captured, has no output. Commands from it are masked the same way before they are returned over MCP.

## Your project and machine

- **No command changes your source files.** Only `mark` and `init` create `.codigest/`, which holds its own `.gitignore` (`*`) so it never shows in `git status`; `init` also adds it to the project's `.gitignore`.
- **Captures are local.** `cdg term path` shows the file. On macOS and Linux it is readable only by you. `cdg term clear` deletes it.
- **Size limits.** The capture store keeps the last 200 commands, and each output is cut to 64 KB, so one noisy command cannot fill it.
- **Agents.** MCP tools return the same masked text and refuse paths outside the project. See [MCP Integration](/mcp#limits).
