list=(
  ".turbo"
  "apps/desktop/dist"
  "apps/web/out"
  "apps/web/.next"
  "apps/api/dist"
  "node_modules"
  "target"
  "apps/desktop/node_modules"
  "apps/web/node_modules"
  "apps/api/node_modules"
  "packages/ui/node_modules"
  "packages/icon/node_modules"
)

for dir in "${list[@]}"; do
  if [ -d "$dir" ]; then
    echo "Removing $dir ..."
    rm -rf "$dir"
  else
    echo "$dir does not exist, skipping."
  fi
done
