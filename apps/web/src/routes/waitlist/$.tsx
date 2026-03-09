import { createFileRoute } from '@tanstack/react-router'
import Waitlist from '@/components/waitlist'

export const Route = createFileRoute('/waitlist/$')({
  component: Waitlist,
})
