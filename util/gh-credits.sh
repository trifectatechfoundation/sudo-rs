#! /bin/bash

# the time of the most recent release
SINCE="$(gh release list --limit 1 --json publishedAt --jq '.[] | .publishedAt')"

cur_contributors() {
    gh pr ls --state merged --limit 100 --json author,mergedAt \
        --jq "unique_by(.author.login) | sort_by(.mergedAt) | .[] | select(.mergedAt >= \"$SINCE\") | .author.login" | \
        grep -v 'app/dependabot'
}

prev_contributors() {
    gh pr ls --state merged --limit 10000 --json author,mergedAt \
        --jq ".[] | select(.mergedAt < \"$SINCE\") | .author.login"
}

cur_issuers() {
    gh issue ls --state closed --json author,closedAt,stateReason \
        --jq "unique_by(.author.login) | sort_by(.closedAt) | .[] | select(.stateReason == \"COMPLETED\" and .closedAt >= \"$SINCE\") | .author.login" | \
        grep -v 'app/dependabot'
}

fmt_handles() {
    sed 's/^/@/' | tr '\n' ',' | sed 's/,@/, @/g;s/,$//'
}

# cache the results
CONTRIBUTORS="$(cur_contributors)"
OLD_CONTRIBUTORS="$(prev_contributors)"

echo "###### Contributors for this release"

# create a list of contributors, where we separate new from repeat contributors

AGAIN=$(grep -Fxf <(echo "$OLD_CONTRIBUTORS") <(echo "$CONTRIBUTORS") | fmt_handles)
FRESH=$(grep -v -Fxf <(echo "$OLD_CONTRIBUTORS") <(echo "$CONTRIBUTORS") | fmt_handles)
echo "Merged pull requests: $AGAIN${FRESH:+${AGAIN:+, }*new contributors:* $FRESH}"

# this list is for people who have only opened issues *only*

REPORTERS=$(grep -v -Fxf <(echo "$CONTRIBUTORS") <(cur_issuers) | fmt_handles)
echo "Closed issues opened by: $REPORTERS"
