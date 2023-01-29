#!/bin/sh
# Check if we need to refresh the metadata cache, and signal back to the Github job if so
timestamp() {
    date -r /var/cache/dnf/last_makecache +%s.%N
}

echo "::group::Refreshing metadata"
if [[ -e /var/cache/dnf/last_makecache ]]; then
    old_time=$(timestamp)
fi
dnf makecache --timer
new_time=$(timestamp)

echo time=$new_time >> $GITHUB_OUTPUT
if [[ $old_time == $new_time ]]; then
    echo refreshed=0 >> $GITHUB_OUTPUT
else
    echo refreshed=1 >> $GITHUB_OUTPUT
fi

# vim: ts=4 sts=4 sw=4 tw=100 expandtab
