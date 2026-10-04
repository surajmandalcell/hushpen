import { act } from "react";
import { render } from "@testing-library/react";
import App from "../App";

type AppProps = Parameters<typeof App>[0];

// The app starts device enumeration and bridge reads on mount. Rendering inside an
// async act() flushes those resolved promises so their state updates are not reported
// as happening outside act().
export async function renderApp(props: AppProps = {}) {
  let result: ReturnType<typeof render> | undefined;

  await act(async () => {
    result = render(<App {...props} />);
  });

  return result as ReturnType<typeof render>;
}
