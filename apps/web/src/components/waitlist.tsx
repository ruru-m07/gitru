const Waitlist = () => {
  return (
    <div className="max-w-[600px] max-h-[750px] h-full w-full mx-1">
      <span className="mono">
        {"$ "}
        <span className="text-muted-foreground">ssh</span> gitru@ruru.build
        <span className="h-full px-[5px] bg-primary ml-1 animate-pulse"></span>
      </span>
    </div>
  );
};

export default Waitlist;
