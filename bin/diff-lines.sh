#
# Filters any git diff by line added.
#
# If you are using BASH instead of ZSH, replace
# match[2] by BASH_REMATCH[2] in all occurencies.
#
# Usage:
#   $ git diff ffcfd5..76ebb8 | diff-lines | grep ".rb"
#     engine/tests/fixtures/app/app.rb:1
#     engine/tests/fixtures/app/app.rb:2
#     engine/tests/fixtures/app/app.rb:3
#

diff-lines() {
    local path=
    local line=
    while read; do
        if [[ $REPLY =~ '\+\+\+\ (b\/)?([^[:blank:]@@]+)' ]]; then
            path=${match[2]}
        elif [[ $REPLY =~ '@@\ -[0-9]+(,[0-9]+)?\ \+([0-9]+)(,[0-9]+)?\ @@.*' ]]; then
            line=${match[2]}
        elif [[ $REPLY =~ '^(\033\[[0-9;]*m)*([\ +-])' ]]; then
            if [[ ${match[2]} != - ]]; then
                #echo "$path:$line:$REPLY"
                echo "$path:$line"
                ((line++))
            fi
        fi
    done
}
