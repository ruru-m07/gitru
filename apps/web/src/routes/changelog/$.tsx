import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/changelog/$')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/changelog/$"!</div>
}
