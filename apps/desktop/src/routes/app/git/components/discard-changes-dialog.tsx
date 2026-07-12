import { Button } from "@gitru/ui/components/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@gitru/ui/components/dialog";
import { Kbd } from "@gitru/ui/components/kbd";
import { CircleAlertIcon, Undo2 } from "lucide-react";
import { memo, useCallback, useState } from "react";
import { toast } from "sonner";
import { useGitDiscard } from "@/hooks";

export const DiscardChangesDialog = memo(function DiscardChangesDialog({
  filePaths,
  label,
}: {
  filePaths: string[];
  label?: string;
}) {
  const [open, setOpen] = useState(false);
  const [isDeleteLoading, setIsDeleteLoading] = useState(false);

  const { mutateAsync: discardChanges } = useGitDiscard();

  const isBulkDiscard = filePaths.length > 1;
  const titleLabel =
    label ??
    (isBulkDiscard
      ? `${filePaths.length} visible files`
      : (filePaths[0]?.split("/").pop() ?? "selected changes"));

  const handleOpenChange = useCallback((newOpen: boolean) => {
    setOpen(newOpen);
  }, []);

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogTrigger
        render={
          <Button
            onClick={(e) => {
              e.stopPropagation();
              setOpen(true);
            }}
            size={"icon-sm"}
            variant={"ghost"}
          />
        }
      >
        <Undo2 size={20} strokeWidth={1.25} />
      </DialogTrigger>
      <DialogContent className="min-w-150">
        <div className="flex flex-col items-center gap-2">
          <div
            className="flex size-9 shrink-0 items-center justify-center rounded-full border"
            aria-hidden="true"
          >
            <CircleAlertIcon
              className="opacity-80 text-destructive"
              size={30}
            />
          </div>
          <DialogHeader>
            <DialogTitle className="sm:text-center">
              Discard changes to{" "}
              <span className="font-semibold text-destructive">
                {titleLabel}
              </span>
              ?
            </DialogTitle>
            <DialogDescription className="sm:text-center">
              This action cannot be undone. Only the currently visible selected
              changes will be permanently lost.
            </DialogDescription>
          </DialogHeader>
        </div>

        <DialogFooter className="grid grid-cols-2 p-3!">
          <DialogClose
            render={
              <Button
                type="button"
                variant="outline"
                className="flex-1"
                size={"lg"}
                onClick={(e) => {
                  e.stopPropagation();
                }}
                disabled={isDeleteLoading}
              />
            }
          >
            Cancel
            <Kbd>Esc</Kbd>
          </DialogClose>
          <Button
            type="button"
            className="flex-1"
            variant={"destructive"}
            disabled={isDeleteLoading}
            size={"lg"}
            onClick={async (e) => {
              e.stopPropagation();
              setIsDeleteLoading(true);

              try {
                await discardChanges({
                  filePath: isBulkDiscard ? filePaths : (filePaths[0] ?? ""),
                });
              } catch (error) {
                toast.error("Unable to discard changes");
              } finally {
                setIsDeleteLoading(false);
                setOpen(false);
              }
            }}
          >
            Discard
            <Kbd className="bg-background/30 text-background">
              {/* <CornerDownLeft /> */}↵
            </Kbd>
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});
