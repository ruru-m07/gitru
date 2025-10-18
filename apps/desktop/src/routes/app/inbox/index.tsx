import { FileWatcherDashboard } from '@/components/fileWatcher/FileWatcherDashboard'
import { createFileRoute, Link } from '@tanstack/react-router'

export const Route = createFileRoute('/app/inbox/')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>
    <Link to='/auth/onboarding'>
      Go to Onboarding
    </Link>
  </div>
}
