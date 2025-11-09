list=(
  ".turbo"
  "node_modules"
  "apps/desktop/dist"
  "apps/desktop/node_modules"
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
