# Codigest terminal capture hook for bash (4+) and zsh. Installed by `cdg term install`.
#
# Enter only records the line (bind -x on bash, the accept-line widget on zsh);
# the line itself is left alone, so the screen and the history show it as typed
# and it runs normally in the current shell. Right before it runs (DEBUG trap on
# bash, preexec on zsh) the shell's stdout and stderr go through tee into a
# private temp file; before the next prompt (PROMPT_COMMAND / precmd) they are
# restored and the file is handed to `cdg term ingest` in a background process
# (redaction happens there, before anything is stored).
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
# A background job would hold the pipe open and the prompt would wait for it;
# a heredoc body arrives as continuation lines.
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
    # The recorded line would show `!!`-style history expansion unexpanded.
    if [ "$__cdg_shell" = zsh ]; then [[ -o banghist && $1 == *!* ]] && return 1
    else [[ $- == *H* && $1 == *!* ]] && return 1; fi
    # Incomplete input (open loop, quote...) goes through untouched to the continuation prompt.
    if [ "$__cdg_shell" = zsh ]; then zsh -f -n -c "$1" 2>/dev/null
    else BASH_ENV= "$BASH" -n -c "$1" 2>/dev/null; fi
}

# Opens the private file and the tee that copies into it. bash calls this on Enter,
# outside any pipeline: a process substitution made while bash sets up a pipeline
# joins that job, which then waits for tee while tee waits for the shell.
__cdg_open() {
    __cdg_file=$__cdg_dir/$$.$RANDOM$RANDOM
    printf '%s\0' "$1" >| "$__cdg_file"  # ingest reads the command from here, output follows
    __cdg_now; __cdg_t0=$__cdg_t
    # tee ignores Ctrl+C so it keeps recording what the interrupted program prints last.
    if [ "$__cdg_shell" = zsh ]; then
        # zsh does not report the pid of a process substitution; tee's exit removes this marker.
        : >| "$__cdg_file.t"
        exec {__cdg_w}> >(trap '' INT; tee -a -- "$__cdg_file"; rm -f -- "$__cdg_file.t")
    else
        __cdg_bg=$!
        exec {__cdg_w}> >(trap '' INT; exec tee -a -- "$__cdg_file")
        __cdg_pid=
        [ "$!" != "$__cdg_bg" ] && __cdg_pid=$!
    fi
}

# Points the shell's stdout and stderr at tee until the next prompt.
__cdg_start() {
    __cdg_unbuf=${PYTHONUNBUFFERED-}
    __cdg_unbuf_set=${PYTHONUNBUFFERED+1}
    export PYTHONUNBUFFERED=1  # piped Python would otherwise block-buffer live output
    __cdg_now; __cdg_t0=$__cdg_t
    exec {__cdg_o}>&1 {__cdg_e}>&2 >&$__cdg_w 2>&1 {__cdg_w}>&-
    __cdg_w= __cdg_busy=1
}

__cdg_post() {
    local s=$? i=0 sig=
    [ -n "${__cdg_file-}" ] || return $s
    __cdg_now
    if [ -n "$__cdg_busy" ]; then
        exec >&$__cdg_o 2>&$__cdg_e {__cdg_o}>&- {__cdg_e}>&-
        if [ -n "$__cdg_unbuf_set" ]; then export PYTHONUNBUFFERED=$__cdg_unbuf; else unset PYTHONUNBUFFERED; fi
        __cdg_busy=
    else
        exec {__cdg_w}>&-  # the line ran no simple command (a function definition)
        __cdg_w=
    fi
    # Let tee print the last output before the prompt. A stopped job (Ctrl+Z) still
    # holds the pipe, so do not wait for it.
    (( s > 128 )) && sig=$(kill -l $s 2>/dev/null)
    case $sig in
        TSTP | STOP | TTIN | TTOU) ;;
        *)  if [ "$__cdg_shell" = zsh ]; then
                while [[ -e $__cdg_file.t ]] && (( i++ < 200 )); do zselect -t 1 2>/dev/null; done
            elif [ -n "$__cdg_pid" ]; then
                wait "$__cdg_pid" 2>/dev/null
            fi ;;
    esac
    ( "$__cdg_exe" term ingest --file="$__cdg_file" --exit-code="$s" --cwd="$PWD" \
        --shell="$__cdg_shell" --start="$__cdg_t0" --end="$__cdg_t" </dev/null >/dev/null 2>&1 & )
    __cdg_file=
    return $s
}

__cdg_file= __cdg_w= __cdg_busy=
if [ "$__cdg_shell" = zsh ]; then
    zmodload zsh/zselect 2>/dev/null
    __cdg_accept() {
        __cdg_line=
        [[ -z $PREBUFFER ]] && __cdg_wants "$BUFFER" && __cdg_line=$BUFFER
        zle __cdg_orig_accept
    }
    __cdg_preexec() {
        [ -n "$__cdg_line" ] || return 0
        __cdg_open "$__cdg_line"
        __cdg_line=
        __cdg_start
    }
    if [ -z "${__cdg_hooked-}" ]; then
        __cdg_hooked=1
        zle -A accept-line __cdg_orig_accept
        zle -N accept-line __cdg_accept
        # Last, so other hooks' output (title escapes) stays out of the capture.
        preexec_functions+=(__cdg_preexec)
        # First, to see the command's exit status.
        precmd_functions=(__cdg_post $precmd_functions)
    fi
else
    # A subshell runs without the DEBUG trap, so a line starting with one would be
    # recorded without its output.
    __cdg_skip+='|^[[:space:]]*\(([^(]|$)'
    __cdg_mark() {
        if [ -z "${__cdg_cont-}" ] && __cdg_wants "$READLINE_LINE"; then __cdg_open "$READLINE_LINE"; fi
        # Any further Enter before the next prompt is a continuation line.
        __cdg_cont=1
    }
    # Runs before every simple command. The first one of a recorded line starts the
    # capture; reaching PROMPT_COMMAND first means the line had none.
    __cdg_debug() {
        [ -n "$__cdg_w" ] && [ "$BASH_COMMAND" != __cdg_prompt ] && __cdg_start
        return 0
    }
    __cdg_prompt() {
        __cdg_post
        local s=$?
        __cdg_cont=
        return $s
    }
    if [ -z "${__cdg_hooked-}" ]; then
        __cdg_hooked=1
        # Keep a DEBUG trap set earlier (starship, bash-preexec); ours runs last. bash hides
        # the current DEBUG trap inside a sourced file or function, so add ours from the
        # first prompt, at top level.
        __cdg_init='eval "__cdg_m=($(trap -p DEBUG))"; trap "${__cdg_m[2]:+${__cdg_m[2]}; }__cdg_debug" DEBUG; unset __cdg_m __cdg_init'
        PROMPT_COMMAND="__cdg_prompt;eval \"\${__cdg_init-}\"${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
        for __cdg_m in emacs vi-insert vi-command; do
            bind -m "$__cdg_m" -x '"\C-x\C-c": __cdg_mark'
            bind -m "$__cdg_m" '"\C-m": "\C-x\C-c\C-j"'
        done
        unset __cdg_m
    fi
fi

# Lets `cdg term` tell whether this shell is hooked.
export CODIGEST_HOOK=1
