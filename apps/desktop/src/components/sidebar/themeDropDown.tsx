'use client'

import { Monitor, Moon, Sun } from 'lucide-react'

import {
  DropdownMenuGroup,
  DropdownMenuPortal,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger
} from '@noutify/ui/components/dropdown-menu'
import { useTheme } from 'next-themes'

const ThemeDropDown = () => {
  const { theme, setTheme } = useTheme()

  return (
    <DropdownMenuSub>
      <DropdownMenuSubTrigger>
        {theme === 'light' && (
          <Sun size={16} strokeWidth={2} className="opacity-60" aria-hidden="true" />
        )}
        {theme === 'dark' && (
          <Moon size={16} strokeWidth={2} className="opacity-60" aria-hidden="true" />
        )}
        {theme === 'system' && (
          <Monitor size={16} strokeWidth={2} className="opacity-60" aria-hidden="true" />
        )}
        Theme
      </DropdownMenuSubTrigger>
      <DropdownMenuPortal>
        <DropdownMenuSubContent>
          <DropdownMenuRadioGroup value={theme} onValueChange={setTheme}>
            <DropdownMenuGroup>
              <DropdownMenuRadioItem className="gap-3" value="light">
                <Sun size={16} strokeWidth={2} className="opacity-60" aria-hidden="true" />
                <span>Light</span>
              </DropdownMenuRadioItem>
              <DropdownMenuRadioItem className="gap-3" value="dark-classic">
                <Moon size={16} strokeWidth={2} className="opacity-60" aria-hidden="true" />
                <span>Dark</span>
              </DropdownMenuRadioItem>
              <DropdownMenuRadioItem className="gap-3" value="system">
                <Monitor size={16} strokeWidth={2} className="opacity-60" aria-hidden="true" />
                <span>System</span>
              </DropdownMenuRadioItem>
            </DropdownMenuGroup>
          </DropdownMenuRadioGroup>
        </DropdownMenuSubContent>
      </DropdownMenuPortal>
    </DropdownMenuSub>
  )
}

export default ThemeDropDown
