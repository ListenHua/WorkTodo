import {
  buttonDark,
  cardDark,
  checkboxDark,
  commonDark,
  inputDark,
  messageDark,
  modalDark,
  selectDark,
  type GlobalTheme,
} from 'naive-ui'

// Include the themes used by the app; each component carries its own internal peers.
export const appDarkTheme: GlobalTheme = {
  name: 'dark',
  common: commonDark,
  Button: buttonDark,
  Card: cardDark,
  Checkbox: checkboxDark,
  Input: inputDark,
  Message: messageDark,
  Modal: modalDark,
  Select: selectDark,
}
