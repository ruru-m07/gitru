import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/download/$')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/download/$"!</div>
}
