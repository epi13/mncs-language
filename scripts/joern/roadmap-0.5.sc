import io.shiftleft.semanticcpg.language.*

@main def roadmap05(cpgFile: String): Unit = {
  importCpg(cpgFile)

  println("MNCS_LANGUAGE_ROADMAP_0_5")
  val targets = List(
    "validate_operation",
    "evaluate_integer",
    "execute_operation",
    "execute_instruction",
    "integer_no_overflow_promise",
    "evaluate",
    "lower_instruction",
    "emit_inst",
    "emit_integer",
    "jit_scalar",
    "evaluate_candidate_command",
    "rust_control_comparison",
    "compare"
  )
  targets.foreach { name =>
    val methods = cpg.method.nameExact(name).filter(_.filename.endsWith(".rs")).l
    methods.sortBy(method => (method.filename, method.lineNumber.getOrElse(-1))).foreach { method =>
      val callers = method.callIn.method.name.l.distinct.sorted.mkString(",")
      val callees = method.callOut.name.l.distinct.sorted.mkString(",")
      val controls = method.controlStructure.controlStructureType.l
        .groupBy(identity).view.mapValues(_.size).toMap.toSeq.sortBy(_._1).mkString(",")
      println(s"METHOD|$name|file=${method.filename}|line=${method.lineNumber.getOrElse(-1)}|callers=$callers|callees=$callees|controls=$controls")
    }
  }

  cpg.call
    .filter(_.file.name.headOption.exists(path =>
      path.endsWith("body.rs") || path.endsWith("execution.rs") ||
      path.endsWith("ssa_execution.rs") || path.endsWith("promises.rs") ||
      path.endsWith("scalar.rs") || path.endsWith("llvm.rs") ||
      path.endsWith("c11.rs") || path.endsWith("cranelift_backend.rs") ||
      path.endsWith("lower.rs") || path.endsWith("refinement.rs") ||
      path.endsWith("experiment.rs") ||
      path.endsWith("mncs-cli/src/main.rs")
    ))
    .filter(call => call.name.matches(
      "validate.*|evaluate.*|integer_.*promise|lower_.*|emit_.*|execute.*|promot.*|identity_is_valid|seal"
    ))
    .map(call => call.file.name.headOption.getOrElse("?") + ":" + call.method.name + ":" + call.name + ":" + call.lineNumber.getOrElse(-1))
    .l.sorted.foreach(value => println(s"ROADMAP_BOUNDARY|$value"))
}
