import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/app/inbox/')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/app/inbox/"!</div>
}
