import { Button } from "@gitru/ui/components/button";
import {
  CommandPanel,
  type CommandViewConfig,
} from "@gitru/ui/components/command";
import {
  Field,
  FieldDescription,
  FieldLabel,
} from "@gitru/ui/components/field";
import { Input } from "@gitru/ui/components/input";
import { GitBranch, Pencil } from "lucide-react";
import { type FormEvent, useState } from "react";
import { toast } from "sonner";
import { useGitRenameBranch } from "@/hooks";
import { simplifyBranchName } from "./create-branch";

export interface RenameBranchProps {
  branchName: string;
}

export function useRenameBranchView(): CommandViewConfig<
  "rename-branch",
  RenameBranchProps
> {
  return {
    id: "rename-branch",
    input: { render: () => null, autoFocus: false },
    header: () => <div />,
    render: ({ props, navigate, close }) => (
      <RenameBranchForm
        key={props.branchName}
        branchName={props.branchName}
        close={close}
        navigateBack={navigate.back}
      />
    ),
  };
}

function RenameBranchForm({
  branchName,
  close,
  navigateBack,
}: {
  branchName: string;
  close: () => void;
  navigateBack: () => void;
}) {
  const renameBranch = useGitRenameBranch();
  const [newName, setNewName] = useState("");
  const normalizedName = simplifyBranchName(newName);
  const canRename = normalizedName.length > 0 && normalizedName !== branchName;

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!canRename) return;
    const result = await renameBranch.mutateAsync({
      branch: branchName,
      newName: normalizedName,
    });
    toast.success(result);
    close();
  };

  return (
    <CommandPanel className="p-4">
      <form className="flex flex-col gap-4" onSubmit={submit}>
        <div className="flex items-center gap-2">
          <Pencil className="size-4" />
          <p className="font-medium">Rename branch</p>
        </div>
        <Field name="branch-name">
          <FieldLabel>New branch name</FieldLabel>
          <Input
            autoFocus
            type="text"
            value={newName}
            onChange={(event) => setNewName(event.target.value)}
            placeholder={branchName}
            spellCheck={false}
          />
          <FieldDescription>
            Slashes and Unicode are supported. The upstream is preserved.
          </FieldDescription>
        </Field>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={navigateBack}>
            Back
          </Button>
          <Button type="submit" disabled={!canRename || renameBranch.isPending}>
            <GitBranch />
            Rename
          </Button>
        </div>
      </form>
    </CommandPanel>
  );
}
