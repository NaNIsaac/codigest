# Time the bash capture hook: what one command costs with and without it.
#
#   bash --norc --noprofile -i benchmarks/hook_overhead.sh [runs] > /dev/null
#
# -i because the hook only loads in an interactive shell; results go to stderr. Each run
# calls the hook's steps for one typed line in the order bash runs them (Enter, the DEBUG
# trap, the command, PROMPT_COMMAND), with captures in a temporary CODIGEST_HOME.
#   wants     the checks on the typed line, including the `bash -n` syntax check
#   command   `echo` with the whole hook around it against `echo` alone
#   lines     20,000 output lines through the tee against without it
# `cdg term ingest` then runs in the background; the script waits for it between runs,
# outside the timed part.

runs=${1:-60}
export CODIGEST_HOME=${TMPDIR:-/tmp}/codigest-bench-$$
mkdir -p "$CODIGEST_HOME"
. "$(dirname "${BASH_SOURCE[0]}")/../src/codigest/shell/hook.sh" 2>/dev/null
[ -n "${__cdg_exe-}" ] || { echo "hook did not load (cdg on PATH? interactive?)" >&2; exit 1; }

settle() { while compgen -G "$__cdg_dir/*" >/dev/null; do sleep 0.05; done; }
stat_ms() {  # median and p95 of microsecond values on stdin
    sort -n | awk '{ v[NR] = $1 } END {
        m = NR % 2 ? v[(NR + 1) / 2] : (v[NR / 2] + v[NR / 2 + 1]) / 2
        p = v[int(0.95 * NR + 0.999)]
        printf "%.2f | %.2f | %d", m / 1000, p / 1000, NR }'
}

line='ls -la src | head -n 3'
hooked() {  # $1: the command to run, as the typed line would
    __cdg_wants "$line" && __cdg_open "$line"
    [ -n "$__cdg_w" ] && __cdg_start
    eval "$1"
    __cdg_post
}

declare -a wants plain hook added lplain lhook
hooked 'echo warm'; settle
for ((i = 0; i < runs; i++)); do
    t0=${EPOCHREALTIME/[.,]/}; __cdg_wants "$line"; t1=${EPOCHREALTIME/[.,]/}; wants+=($((t1 - t0)))
    t0=${EPOCHREALTIME/[.,]/}; echo "run $i"; t1=${EPOCHREALTIME/[.,]/}; plain+=($((t1 - t0)))
    t0=${EPOCHREALTIME/[.,]/}; hooked 'echo "run $i"'; t1=${EPOCHREALTIME/[.,]/}; hook+=($((t1 - t0)))
    added+=($((hook[i] - plain[i])))
    settle
done
lines_cmd='for ((j = 0; j < 20000; j++)); do echo "line $j"; done'
for ((i = 0; i < (runs / 6 > 5 ? runs / 6 : 5); i++)); do
    t0=${EPOCHREALTIME/[.,]/}; eval "$lines_cmd"; t1=${EPOCHREALTIME/[.,]/}; lplain+=($((t1 - t0)))
    t0=${EPOCHREALTIME/[.,]/}; hooked "$lines_cmd"; t1=${EPOCHREALTIME/[.,]/}; lhook+=($((t1 - t0)))
    settle
done

stored=$(wc -l < "$CODIGEST_HOME/term/history.jsonl" 2>/dev/null || echo 0)
rm -rf "$CODIGEST_HOME"
{
    echo "bash $BASH_VERSION, cdg: $__cdg_exe"
    echo "runs: $runs (${#lhook[@]} for 20k lines); entries stored: $stored"
    echo
    echo '| measurement | median ms | p95 ms | n |'
    echo '|---|---:|---:|---:|'
    echo "| wants | $(printf '%s\n' "${wants[@]}" | stat_ms) |"
    echo "| command plain | $(printf '%s\n' "${plain[@]}" | stat_ms) |"
    echo "| command hooked | $(printf '%s\n' "${hook[@]}" | stat_ms) |"
    echo "| command added | $(printf '%s\n' "${added[@]}" | stat_ms) |"
    echo "| 20k lines plain | $(printf '%s\n' "${lplain[@]}" | stat_ms) |"
    echo "| 20k lines hooked | $(printf '%s\n' "${lhook[@]}" | stat_ms) |"
} >&2
