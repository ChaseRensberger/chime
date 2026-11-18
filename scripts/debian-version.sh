set -eu
package=$1
semver=${2#v}
actual=$(dpkg-deb -f "$package" Version)
test "$actual" = "$semver"
case "$semver" in
  *-*)
    stable=${semver%%-*}
    debversion="$stable~${semver#*-}"
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    dpkg-deb -R "$package" "$work/package"
    sed -i "s/^Version: .*/Version: $debversion/" "$work/package/DEBIAN/control"
    dpkg-deb --root-owner-group --build "$work/package" "$work/updated.deb"
    test "$(dpkg-deb -f "$work/updated.deb" Version)" = "$debversion"
    dpkg --compare-versions "$debversion" lt "$stable"
    mv "$work/updated.deb" "$package"
    ;;
esac
