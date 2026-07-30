export const formatNumber = (num: number, locale: string = "en-US") => {
  return new Intl.NumberFormat(locale).format(num);
};
