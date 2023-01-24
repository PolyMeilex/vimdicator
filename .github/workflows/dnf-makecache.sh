#!/bin/sh
# Check if we need to refresh the metadata cache, and signal back to the Github job if so
echo "::group::Refreshing metadata"
if [[ -e /var/cache/dnf/last_makecache ]]; then
    old_hash=$(sha256sum /var/cache/dnf/last_makecache)
fi
dnf makecache --timer
new_hash=$(sha256sum /var/cache/dnf/last_makecache)

echo hash=$(grep -Po '^[0-9a-f]+' <<< $new_hash) >> $GITHUB_OUTPUT
if [[ $old_hash == $new_hash ]]; then
    echo refreshed=0 >> $GITHUB_OUTPUT
else
    echo refreshed=1 >> $GITHUB_OUTPUT
fi

# vim: ts=4 sts=4 sw=4 tw=100 expandtab
