import { Mascot } from "@gitru/mascot";

const DownloadPage = () => {
  return (
    <div className="h-screen w-screen flex flex-col font-mono items-center justify-center **:data-[name='mascot-svg']:size-48">
      <Mascot />
    </div>
  );
};

export default DownloadPage;
