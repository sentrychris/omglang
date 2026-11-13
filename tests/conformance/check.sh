# Conformance corpus driver. Sourced by tests/smoke.sh (portable
# backends) and tests/conformance.sh (native backends); not run directly.
#
# The corpus is two kinds of program:
#
#   tests/conformance/NAME.omg        must exit 0 and print exactly
#                                     NAME.expected
#   tests/conformance/fail/NAME.omg   must print NAME.expected, then die
#                                     with exit code 1 and an error of
#                                     the kind named on its
#                                     `# expect: Kind` line
#
# Only the error kind is compared, never the wording after the colon:
# the wording is allowed to differ between backends.
#
# A failing program may also have a NAME.traceback file: the `File ...,
# line N, in PROC` lines its traceback must consist of, with each path
# cut down to the file's name. Give one to a program whose point is
# where the traceback points. Leave it out where a tail call is in the
# picture, because back ends differ on which calls they turn into
# jumps, and a frame that was replaced is not in the traceback.
#
# The sourcing script provides `pass NAME` and `fail NAME DETAIL`, and a
# function `conf_run BACKEND FILE` that runs FILE on BACKEND, writing
# stdout to $CONF_DIR/out and stderr to $CONF_DIR/err and setting
# CONF_RC to the exit code. A backend that cannot even build the program
# should leave its build log in $CONF_DIR/err and set a non-zero CONF_RC.

conf_norm() { tr -d '\r'; }

# The frames of a traceback on stdin, paths reduced to file names.
conf_frames() { conf_norm | grep '^  File "' | sed 's#File ".*[/\\]#File "#'; }

# conformance_backend BACKEND LABEL: run the whole corpus on one backend.
conformance_backend() {
    local backend="$1" label="$2" f name expected actual kind last frames
    for f in tests/conformance/*.omg; do
        name=$(basename "$f" .omg)
        expected=$(conf_norm < "${f%.omg}.expected")
        : > "$CONF_DIR/out"; : > "$CONF_DIR/err"
        conf_run "$backend" "$f"
        actual=$(conf_norm < "$CONF_DIR/out")
        if [ "$CONF_RC" = 0 ] && [ "$actual" = "$expected" ]; then
            pass "$name ($label)"
        else
            fail "$name ($label)" "exit code $CONF_RC
$(diff <(echo "$expected") <(echo "$actual") | head -n 12)
$(conf_norm < "$CONF_DIR/err" | tail -n 6)"
        fi
    done
    for f in tests/conformance/fail/*.omg; do
        name=$(basename "$f" .omg)
        expected=$(conf_norm < "${f%.omg}.expected")
        kind=$(sed -n 's/^# expect: //p' "$f" | conf_norm | head -n 1)
        : > "$CONF_DIR/out"; : > "$CONF_DIR/err"
        conf_run "$backend" "$f"
        actual=$(conf_norm < "$CONF_DIR/out")
        last=$(conf_norm < "$CONF_DIR/err" | tail -n 1)
        case "$last" in
            "$kind:"*|"$kind")
                if [ "$CONF_RC" = 1 ] && [ "$actual" = "$expected" ]; then
                    pass "fails with $kind: $name ($label)"
                else
                    fail "fails with $kind: $name ($label)" "exit code $CONF_RC (expected 1)
stdout: $actual"
                fi
                if [ -f "${f%.omg}.traceback" ]; then
                    frames=$(conf_frames < "$CONF_DIR/err")
                    if [ "$frames" = "$(conf_norm < "${f%.omg}.traceback")" ]; then
                        pass "traceback positions: $name ($label)"
                    else
                        fail "traceback positions: $name ($label)" "$(diff \
                            <(conf_norm < "${f%.omg}.traceback") <(echo "$frames"))"
                    fi
                fi ;;
            *)
                fail "fails with $kind: $name ($label)" "exit code $CONF_RC
stdout: $actual
last line of stderr: $last" ;;
        esac
    done
}
