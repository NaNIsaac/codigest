# Codigest terminal capture hook for bash (4+) and zsh. Installed by `cdg term install`.
#
# On Enter, the accepted line is rewritten to `__cdg_pre <line>; __cdg_run`.
# `__cdg_run` is an alias, so the line runs through `eval` in the current
# shell: `cd`, variables, functions and `$1` behave exactly as typed. Output is
# teed into a private temp file and handed to `cdg term ingest` in a background
# process (redaction happens there, before anything is stored).
#
# Opt out for one command: start the line with a space.
# Opt out entirely: export CODIGEST_NOCAP=1

[ -n "${CODIGEST_NOCAP-}" ] && return 0
case $- in *i*) ;; *) return 0 ;; esac
if [ -n "${ZSH_VERSION-}" ]; then
    __cdg_shell=zsh
    zmodload zsh/datetime 2>/dev/null
elif [ -n "${BASH_VERSION-}" ] && [ "${BASH_VERSINFO[0]}" -ge 4 ]; then
    __cdg_shell=bash
else
    return 0
fi
__cdg_exe=$(command -v cdg 2>/dev/null) || return 0

# Raw output may hold secrets until ingest redacts it: keep it owner-only.
__cdg_dir=${TMPDIR:-/tmp}/codigest-${UID:-0}
[ -d "$__cdg_dir" ] || mkdir -m 700 "$__cdg_dir" 2>/dev/null
{ [ -d "$__cdg_dir" ] && [ -O "$__cdg_dir" ] && [ ! -L "$__cdg_dir" ]; } || return 0

# Denylist heuristic for programs that need a real TTY (editors, pagers,
# REPLs, ssh). Extend with CODIGEST_CAP_SKIP (an extended regex).
__cdg_skip='^[[:space:]]*(vim?|nvim|nano|emacs|less|more|man|ssh|htop|btop|top|fzf|lazygit|tmux|screen|claude|codex|wsl|exit|exec|fg|bg)(\.exe)?([[:space:]]|$)'
__cdg_skip+='|^[[:space:]]*(cdg|codigest)(\.exe)?[[:space:]]+(term|mcp)([[:space:]]|$)'
__cdg_skip+='|^[[:space:]]*(python3?|py|ipython|node|pwsh|powershell|bash|zsh|sh|fish|cmd)(\.exe)?[[:space:]]*$'
__cdg_skip+='|^[[:space:]]*git[[:space:]]+(rebase[[:space:]]+-i|add[[:space:]]+-[pi]|mergetool)'
# A background job would hold the pipe open; a one-line heredoc reads its body later.
__cdg_skip+='|(^|[^&<>|])&($|[^&>])|(^|[^<])<<([^<]|$)'
__cdg_commit='^[[:space:]]*git[[:space:]]+commit'
__cdg_msg='[[:space:]]-[[:alnum:]]*[mF]'

__cdg_now() {
    if [ -n "${EPOCHREALTIME-}" ]; then __cdg_t=$EPOCHREALTIME
    else printf -v __cdg_t '%(%s)T' -1; fi
}

__cdg_wants() {
    [ -z "${CODIGEST_NOCAP-}" ] || return 1
    case $1 in '' | [[:space:]]*) return 1 ;; esac
    [[ $1 =~ $__cdg_skip ]] && return 1
    [[ -n ${CODIGEST_CAP_SKIP-} && $1 =~ $CODIGEST_CAP_SKIP ]] && return 1
    [[ $1 =~ $__cdg_commit && ! $1 =~ $__cdg_msg ]] && return 1
    # Quoting the line would stop `!!`-style history expansion.
    if [ "$__cdg_shell" = zsh ]; then [[ -o banghist && $1 == *!* ]] && return 1
    else [[ $- == *H* && $1 == *!* ]] && return 1; fi
    # Incomplete input (open loop, quote...) goes through untouched to the continuation prompt.
    if [ "$__cdg_shell" = zsh ]; then zsh -f -n -c "$1" 2>/dev/null
    else BASH_ENV= "$BASH" -n -c "$1" 2>/dev/null; fi
}

__cdg_pre() {
    local s=$?
    __cdg_cmd=$1
    __cdg_file=$__cdg_dir/$$.$RANDOM$RANDOM
    printf '%s\0' "$1" >| "$__cdg_file"  # ingest reads the command from here, output follows
    __cdg_unbuf=${PYTHONUNBUFFERED-}
    __cdg_unbuf_set=${PYTHONUNBUFFERED+1}
    export PYTHONUNBUFFERED=1  # piped Python would otherwise block-buffer live output
    __cdg_bg=$!
    __cdg_now; __cdg_t0=$__cdg_t
    __cdg_busy=1
    return $s
}

__cdg_post() {
    local s=$?
    [ -n "${__cdg_busy-}" ] || return $s
    __cdg_busy=
    __cdg_now
    # bash does not wait for process substitutions; zsh does for a brace group.
    [ "$__cdg_shell" = bash ] && [ "$!" != "$__cdg_bg" ] && wait "$!" 2>/dev/null
    if [ -n "$__cdg_unbuf_set" ]; then export PYTHONUNBUFFERED=$__cdg_unbuf; else unset PYTHONUNBUFFERED; fi
    ( "$__cdg_exe" term ingest --file="$__cdg_file" --exit-code="$s" --cwd="$PWD" \
        --shell="$__cdg_shell" --start="$__cdg_t0" --end="$__cdg_t" </dev/null >/dev/null 2>&1 & )
    return $s
}

# tee ignores Ctrl+C so it keeps recording what the interrupted program prints last.
alias __cdg_run='{ eval "$__cdg_cmd"; } > >(trap "" INT; exec tee -a -- "$__cdg_file") 2>&1; __cdg_post'

if [ "$__cdg_shell" = zsh ]; then
    __cdg_accept() {
        if [[ -z $PREBUFFER ]] && __cdg_wants "$BUFFER"; then
            print -sr -- "$BUFFER"
            BUFFER="__cdg_pre ${(qq)BUFFER}; __cdg_run"
        fi
        zle __cdg_orig_accept
    }
    # Keep the wrapper out of history; the original line was added above.
    __cdg_addhistory() { [[ $1 != '__cdg_pre '* ]]; }
    if [ -z "${__cdg_hooked-}" ]; then
        __cdg_hooked=1
        zle -A accept-line __cdg_orig_accept
        zle -N accept-line __cdg_accept
        zshaddhistory_functions=(__cdg_addhistory $zshaddhistory_functions)
        # Ctrl+C skips the trailing __cdg_post; finish the entry before the prompt.
        precmd_functions=(__cdg_post $precmd_functions)
    fi
else
    __cdg_rewrite() {
        if [ -z "${__cdg_cont-}" ] && __cdg_wants "$READLINE_LINE"; then
            history -s -- "$READLINE_LINE"
            printf -v READLINE_LINE '__cdg_pre %q; __cdg_run' "$READLINE_LINE"
        fi
        # Any further Enter before the next prompt is a continuation line.
        __cdg_cont=1
    }
    __cdg_prompt() {
        __cdg_post
        local s=$?
        __cdg_cont=
        return $s
    }
    if [ -z "${__cdg_hooked-}" ]; then
        __cdg_hooked=1
        HISTIGNORE="__cdg_pre *${HISTIGNORE:+:$HISTIGNORE}"
        PROMPT_COMMAND="__cdg_prompt${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
        for __cdg_m in emacs vi-insert vi-command; do
            bind -m "$__cdg_m" -x '"\C-x\C-c": __cdg_rewrite'
            bind -m "$__cdg_m" '"\C-m": "\C-x\C-c\C-j"'
        done
        unset __cdg_m
    fi
fi

# Lets `cdg term` tell whether this shell is hooked.
export CODIGEST_HOOK=1
