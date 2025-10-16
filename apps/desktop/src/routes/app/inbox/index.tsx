import { FileWatcherDashboard } from '@/components/fileWatcher/FileWatcherDashboard'
import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/app/inbox/')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>
    <FileWatcherDashboard />

    
  </div>
}
