import Blueprint from "./blueprint";

const MascotPage = () => {
  return (
    <div className="w-full">
      <div className="max-w-(--container-width) px-(--container-gutter) mx-auto">
        <div className="flex justify-center w-full">
          <Blueprint className="w-full h-fit" />
        </div>
      </div>
    </div>
  );
};

export default MascotPage;
