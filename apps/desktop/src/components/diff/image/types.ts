export type ImageAssetEntry = {
  absolute_path: string;
  mime: string;
  bytes: number;
  logical_path: string;
};

export type ImageAssetDiff = {
  kind: "Image";
  before?: ImageAssetEntry;
  after?: ImageAssetEntry;
};

export type ImageDimensions = {
  width: number;
  height: number;
};

export type ImageDiffViewProps = {
  before?: ImageAssetEntry;
  after?: ImageAssetEntry;
  width: number;
  height: number;
  beforeUrl?: string;
  afterUrl?: string;
  beforeDimensions: ImageDimensions | null;
  afterDimensions: ImageDimensions | null;
  onBeforeImageLoad: (img: HTMLImageElement) => void;
  onAfterImageLoad: (img: HTMLImageElement) => void;
  reducedMotion: boolean;
};
