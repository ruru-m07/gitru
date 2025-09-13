import "./App.css";

import { Button } from "@noutify/ui/components/button";
import { HashRouter } from "react-router-dom";
import CustomTitleBar from "./CustomTitleBar";

function App() {
	return (
		<HashRouter>
			<main className="w-full">
				<CustomTitleBar restrictedPaths={["/login", "/register", "/welcome"]} />
				<Button> hello </Button>
			</main>
		</HashRouter>
	);
}

export default App;
