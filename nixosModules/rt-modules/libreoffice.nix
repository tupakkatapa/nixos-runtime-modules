# LibreOffice with spell-checking
{ pkgs, ... }: {
  environment.systemPackages = with pkgs; [
    hunspell
    hunspellDicts.en_US
    kdePackages.kcalc
    libreoffice-qt
  ];
}
