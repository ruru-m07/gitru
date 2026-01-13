list=(
  ".turbo"
  "node_modules"
  "apps/desktop/dist"
  "apps/desktop/node_modules"
  "apps/web/node_modules"
  "apps/web/dist"
  "apps/api/node_modules"
  "apps/api/dist"
  "packages/ui/node_modules"
  "packages/icon/node_modules"
  "target"
)

for dir in "${list[@]}"; do
  if [ -d "$dir" ]; then
    echo "Removing $dir ..."
    rm -rf "$dir"
  else
    echo "$dir does not exist, skipping."
  fi
done
