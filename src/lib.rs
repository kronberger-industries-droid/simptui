pub use self::core::*;

mod core {
    use indicatif::{ProgressBar, ProgressStyle};
    use regex::Regex;
    use std::collections::HashMap;
    use std::fs::{self, File};
    use std::io::{self, BufRead, BufReader, Read, Write};
    use std::path::Path;
    use std::process::Command;

    #[derive(Debug, Clone)]
    pub struct Equation {
        pub active: bool,
        pub name: String,
        pub body: String,
    }

    impl Equation {
        pub fn new(active: bool, name: &str, body: &str) -> Self {
            let valid_name = Equation::sanitize_filename(name);
            Equation {
                active,
                name: valid_name,
                body: body.to_string(),
            }
        }

        fn sanitize_filename(name: &str) -> String {
            let re = Regex::new(r"[^a-zA-Z0-9_.]").unwrap();
            let mut sanitized = re.replace_all(name, "_").to_string();
            if sanitized.is_empty() {
                sanitized = "default_equation".to_string();
            }
            sanitized
        }

        pub fn render(
            &self,
            output_dir: &Path,
            color: &str,
            delete_intermediates: bool,
        ) -> io::Result<()> {
            if !self.active {
                // println!("Skipping inactive equation: {}", self.name);
                return Ok(());
            }

            fs::create_dir_all(output_dir)?;

            let latex_source = self.generate_latex(color);
            let tex_file_path = output_dir.join(format!("{}.tex", self.name));

            fs::write(&tex_file_path, latex_source)?;

            let status = Command::new("tectonic")
                .arg(&tex_file_path)
                .arg("--outdir")
                .arg(output_dir)
                .stdout(std::process::Stdio::null()) // Suppress stdout
                .stderr(std::process::Stdio::null()) // Suppress stderr
                .status()?;

            if status.success() {
                // println!("Rendered PDF for {}", self.name);
                self.convert_pdf_to_svg(output_dir)?;

                if delete_intermediates {
                    self.cleanup_intermediate_files(output_dir)?;
                }
            } else {
                eprintln!("Failed to render PDF for {}", self.name);
            }

            Ok(())
        }

        fn convert_pdf_to_svg(&self, output_dir: &Path) -> io::Result<()> {
            let check = Command::new("pdftocairo").arg("-version").output();

            if check.is_err() {
                eprintln!("Error: pdftocairo not found. Please install it to enable PDF to SVG conversion.");
                return Ok(());
            }

            let pdf_file = output_dir.join(format!("{}.pdf", self.name));
            let svg_file = output_dir.join(format!("{}.svg", self.name));

            if !pdf_file.exists() {
                eprintln!("PDF file not found: {}", pdf_file.display());
                return Ok(());
            }

            let status = Command::new("pdftocairo")
                .arg("-svg")
                .arg(&pdf_file)
                .arg(&svg_file)
                .status()?;

            if status.success() {
                //println!("Converted {} to SVG", self.name);
            } else {
                eprintln!("Failed to convert {} to SVG", self.name);
            }

            Ok(())
        }

        fn cleanup_intermediate_files(&self, output_dir: &Path) -> io::Result<()> {
            let tex_file = output_dir.join(format!("{}.tex", self.name));
            let pdf_file = output_dir.join(format!("{}.pdf", self.name));

            fs::remove_file(tex_file).ok();
            fs::remove_file(pdf_file).ok();

            //println!("Intermediate files deleted for {}", self.name);
            Ok(())
        }

        fn generate_latex(&self, color: &str) -> String {
            let color_code = color.trim_start_matches('#');
            format!(
                r#"\documentclass[border=1pt]{{standalone}}
                \usepackage{{amsmath}}
                \usepackage{{xfrac}}
                \usepackage{{gfsneohellenicot}}
                \usepackage{{xcolor}}
                \definecolor{{equationcolor}}{{HTML}}{{{}}}
                \begin{{document}}
                \setbox0\hbox{{\Large \textcolor{{equationcolor}}{{$ {} $}}}}
                \dimen0=12mm
                \ifdim\ht0<\dimen0
                \ht0=\dimen0
                \fi
                \ifdim\dp0<5mm
                \dp0=5mm
                \fi
                \box0
                \end{{document}}"#,
                color_code, self.body
            )
        }
    }

    pub fn ask_confirmation(prompt: &str) -> bool {
        loop {
            print!("{} (y/n): ", prompt);
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim().to_lowercase();

            match input.as_str() {
                "y" | "yes" => return true,
                "n" | "no" => return false,
                _ => {
                    println!("Invalid input. Please enter 'y' or 'n'.");
                }
            }
        }
    }

    pub fn render_equations(
        equations: &[Equation],
        output_dir: &Path,
        color: &str,
        delete_intermediates: bool,
    ) -> io::Result<()> {
        let active_equations: Vec<&Equation> = equations.iter().filter(|eq| eq.active).collect();
        let bar = ProgressBar::new(active_equations.len() as u64);

        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .expect("Error setting template")
                .progress_chars("#>-"),
        );

        for eq in active_equations {
            bar.set_message(format!("Rendering: {}", eq.name));
            eq.render(output_dir, color, delete_intermediates)?;
            bar.inc(1);
        }

        bar.finish_with_message("Rendering complete!");
        Ok(())
    }

    pub fn read_file(path: &Path) -> io::Result<String> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Ok(content)
    }

    pub fn read_csv_file(path: &Path) -> io::Result<Vec<Equation>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut equations = Vec::new();
        let mut name_count: HashMap<String, usize> = HashMap::new();

        for line in reader.lines().skip(1) {
            let line = line?;
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 3 {
                let active = parts[0].trim().eq_ignore_ascii_case("yes");
                let body = parts[1].trim();
                let base_name = if parts[2].trim().is_empty() {
                    "default_equation"
                } else {
                    parts[2].trim()
                };
                let mut name = base_name.to_string();

                let count = name_count.entry(name.clone()).or_insert(0);
                if *count > 0 {
                    name = format!("{}_{}", base_name, count);
                }
                *count += 1;

                let equation = Equation::new(active, &name, body);
                equations.push(equation);
            }
        }
        Ok(equations)
    }

    pub fn detect_file_type(path: &Path) -> &'static str {
        match path.extension().and_then(|s| s.to_str()) {
            Some("csv") => "csv",
            Some("md") | Some("markdown") => "markdown",
            _ => "unknown",
        }
    }

    pub fn parse_markdown(content: &str) -> Vec<Equation> {
        let mut equations = Vec::new();
        let mut name_count: HashMap<String, usize> = HashMap::new();
        let re = Regex::new(r"(?s)(%%(yes|no)?%%)?[\n\r]*\$\$[\n\r]*(.*?)\$\$[\n\r]*(%%(.*?)%%)?")
            .unwrap();

        for cap in re.captures_iter(content) {
            let body = cap.get(3).unwrap().as_str().trim();
            let active = cap.get(2).map_or(true, |m| m.as_str() == "yes");
            let base_name = cap.get(5).map_or("default_equation", |m| m.as_str());
            let mut name = base_name.to_string();

            let count = name_count.entry(name.clone()).or_insert(0);
            if *count > 0 {
                name = format!("{}_{}", base_name, count);
            }
            *count += 1;

            let equation = Equation::new(active, &name, body);
            equations.push(equation);
        }

        equations
    }
}
