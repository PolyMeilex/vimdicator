#!/bin/sh
# Clean unneeded RPM packages out of the dnf metadata directory, without getting rid of any
# downloaded packages that are currently installed
echo "::group::Pruning dnf package cache"
for file in /var/cache/dnf/*/packages/*.rpm; do
    if ! rpm --quiet -q "$(basename "$file" .rpm)"; then
        rm -v "$file"
    fi
done

# vim: ts=4 sts=4 sw=4 tw=100 expandtab
