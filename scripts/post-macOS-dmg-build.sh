full_width=1492
full_height=760

x_position_icons=400
y_position_icons=438

x_position_icons_app_link=$((full_width - x_position_icons))

rm -rf ./dist/*

create-dmg \
  --volname "Noutify" \
  --background "assets/dmg-background.png" \
  --volicon "target/release/bundle/dmg/icon.icns" \
  --window-pos 230 150 \
  --window-size $full_width $full_height \
  --icon-size 200 \
  --icon "noutify.app" $x_position_icons $y_position_icons \
  --app-drop-link $x_position_icons_app_link $y_position_icons \
  "dist/noutify.dmg" \
  "target/release/bundle/macos/"
