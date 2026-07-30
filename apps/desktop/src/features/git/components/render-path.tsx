export const renderPath = (path: string) => {
  const parts = path.split("/");
  const fileName = parts.pop();
  const dir = parts.join("/");

  return (
    <span>
      {dir && (
        <>
          <span className="text-muted-foreground/75">{dir}/</span>
        </>
      )}
      {fileName}
    </span>
  );
};
